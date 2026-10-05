//! The edits, undo, Revert To and Save a Copy in the window under the harness: each is a request
//! of the host (it alone writes the person's file), and the window shows the file as it is until
//! the host's save changes it.

#![allow(clippy::unwrap_used)]

#[path = "../../anyview-pdf/tests/support/mod.rs"]
mod pdf_fixture;
mod support;

use anyview_core::{Axis, ByteLen, Edit, PageIndex, QuarterTurn};
use anyview_ui::{EditRequest, HostRequest, Rewind, TypedText, VersionKey, VersionRow};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use std::sync::Arc;
use support::{Requests, Versions, Wiring, folder, is, rgb, settle, window, wired};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const YELLOW: [u8; 3] = [255, 255, 0];

fn picture(versions: Vec<VersionRow>) -> (tempfile::TempDir, Harness, Requests) {
    let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let wiring = Wiring {
        versions: Some(Arc::new(Versions(versions))),
        ..Wiring::default()
    };
    let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    (dir, harness, requests)
}

fn from_the_palette(harness: &mut Harness, words: &str) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    for letter in words.chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(harness);
    harness.send(Input::key(ShortcutKey::Enter));
    settle(harness);
}

/// What the window asked of its host that is about changing the file.
fn asked(requests: &Requests) -> Vec<HostRequest> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| {
            matches!(
                request,
                HostRequest::Edit(_)
                    | HostRequest::Rewind(_)
                    | HostRequest::RevertTo(_)
                    | HostRequest::SaveCopy(_)
            )
        })
        .cloned()
        .collect()
}

/// Click the sheet's default button, the last of its row.
fn confirm(harness: &mut Harness) {
    let button = harness
        .centre(".viewer-sheet-buttons .ds-button:last-child")
        .expect("the default button");
    harness.send(Input::click(button));
    settle(harness);
}

fn row(key: &'static str, saved_at: u64, size: u64) -> VersionRow {
    VersionRow {
        key: VersionKey::from_static(key),
        saved_at,
        size: ByteLen(size),
    }
}

#[test]
fn rotating_right_asks_the_host_to_save_the_turn_and_the_picture_stays_as_the_file_is() {
    let (_dir, mut harness, requests) = picture(vec![]);
    harness.send(Input::pointer_move(support::middle()));
    harness.advance(std::time::Duration::from_millis(300));
    let button = harness
        .centre("[aria-label=\"Rotate right\"]")
        .expect("the rotate button");
    harness.send(Input::click(button));
    settle(&mut harness);
    assert_eq!(
        asked(&requests),
        [HostRequest::Edit(EditRequest::of_picture(Edit::Rotate(
            QuarterTurn::Quarter
        )))]
    );
    // The window shows the file; the file is turned by the host, and the window then reloads.
    let image = harness.render().unwrap();
    for (name, dx, dy, want) in [
        ("top left", -12.0, -8.0, RED),
        ("top right", 12.0, -8.0, GREEN),
        ("bottom left", -12.0, 8.0, BLUE),
        ("bottom right", 12.0, 8.0, YELLOW),
    ] {
        let got = rgb(&image, dx, dy);
        assert!(is(got, want), "{name}: {got:?} is not {want:?}");
    }
}

#[test]
fn the_palette_flips_a_picture_across_either_axis() {
    // name, what is typed, the edit
    const CASES: &[(&str, &str, Edit)] = &[
        (
            "horizontally",
            "flip horizontal",
            Edit::Flip(Axis::Horizontal),
        ),
        ("vertically", "flip vertical", Edit::Flip(Axis::Vertical)),
    ];
    for (name, words, edit) in CASES {
        let (_dir, mut harness, requests) = picture(vec![]);
        from_the_palette(&mut harness, words);
        assert_eq!(
            asked(&requests),
            [HostRequest::Edit(EditRequest::of_picture(*edit))],
            "{name}"
        );
    }
}

#[test]
fn control_z_undoes_and_shift_control_z_redoes() {
    let (_dir, mut harness, requests) = picture(vec![]);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('z')));
    settle(&mut harness);
    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('z'),
    ));
    settle(&mut harness);
    assert_eq!(
        asked(&requests),
        [
            HostRequest::Rewind(Rewind::Undo),
            HostRequest::Rewind(Rewind::Redo)
        ]
    );
}

#[test]
fn revert_to_lists_the_kept_versions_newest_first_and_choosing_one_asks_for_it() {
    let versions = vec![
        row("k/new", 1_790_000_000, 2_048),
        row("k/old", 1_780_000_000, 1_024),
    ];
    let (_dir, mut harness, requests) = picture(versions);
    from_the_palette(&mut harness, "revert");
    assert_eq!(
        harness.count(".viewer-sheet .ds-radio-group-item"),
        2,
        "a row for each version"
    );
    let listed = harness.text_of(".viewer-sheet").unwrap_or_default();
    assert!(
        listed.find("2026-09-21").unwrap() < listed.find("2026-05-").unwrap(),
        "newest first: {listed}"
    );
    assert!(
        listed.contains("2.0 kB"),
        "and each row says its size: {listed}"
    );
    let older = harness
        .centre(".viewer-sheet .ds-radio-group-item:nth-child(2)")
        .expect("the older row");
    harness.send(Input::click(older));
    settle(&mut harness);
    confirm(&mut harness);
    assert_eq!(
        asked(&requests),
        [HostRequest::RevertTo(VersionKey::from_static("k/old"))]
    );
    assert_eq!(harness.count(".viewer-sheet"), 0, "the sheet is put away");
}

#[test]
fn revert_to_on_a_file_with_no_kept_version_says_so_and_asks_for_nothing() {
    let (_dir, mut harness, requests) = picture(vec![]);
    from_the_palette(&mut harness, "revert");
    let said = harness.text_of(".viewer-sheet").unwrap_or_default();
    assert!(said.contains("no earlier version"), "{said}");
    confirm(&mut harness);
    assert_eq!(harness.count(".viewer-sheet"), 0);
    assert!(asked(&requests).is_empty());
}

#[test]
fn save_a_copy_proposes_a_name_beside_the_file_and_asks_for_it_when_confirmed() {
    let (_dir, mut harness, requests) = picture(vec![]);
    from_the_palette(&mut harness, "save copy");
    assert_eq!(harness.count(".viewer-sheet"), 1, "the sheet is up");
    confirm(&mut harness);
    assert_eq!(
        asked(&requests),
        [HostRequest::SaveCopy(TypedText::new("quadrants copy.png"))]
    );
}

#[test]
fn deleting_and_moving_the_page_on_screen_ask_the_host_for_the_page_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.pdf");
    std::fs::write(&path, pdf_fixture::fixture_bytes()).unwrap();
    let paths: Vec<PathBuf> = vec![std::fs::canonicalize(path).unwrap()];
    let (mut harness, requests) = window(&paths, 0, Appearance::default());
    settle(&mut harness);
    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Down,
    ));
    settle(&mut harness);
    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Backspace,
    ));
    settle(&mut harness);
    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Up,
    ));
    settle(&mut harness);
    assert_eq!(
        asked(&requests),
        [
            HostRequest::Edit(EditRequest::on_page(
                Edit::MovePage {
                    from: PageIndex(0),
                    to: PageIndex(1)
                },
                PageIndex(0)
            )),
            HostRequest::Edit(EditRequest::on_page(
                Edit::DeletePages(
                    anyview_core::PageRange::new(PageIndex(0), PageIndex(0)).unwrap()
                ),
                PageIndex(0)
            )),
        ],
        "the first page moves later and is deleted; it cannot move earlier than the start"
    );
}
