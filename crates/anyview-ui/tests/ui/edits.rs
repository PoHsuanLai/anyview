//! The edits, undo, Revert To and Save a Copy in the window under the harness: each is a request
//! of the host (it alone writes the person's file), and the window shows the file as it is until
//! the host's save changes it.

use crate::pdf_fixture;
use crate::support;

use anyview_core::{
    Adjust, Axis, ByteLen, Edit, PageIndex, PixelLen, PixelRect, PixelSize, QuarterTurn,
};
use anyview_ui::{EditRequest, HostRequest, Rewind, TypedText, VersionKey, VersionRow};
use ds::prelude::{Appearance, ShortcutKey};
use ds::prelude::{Point, Px};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
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

/// The save chord.
fn save(harness: &mut Harness) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('s')));
    settle(harness);
}

fn write_of(adjust: Adjust) -> HostRequest {
    HostRequest::Edit(EditRequest::of_picture(Edit::Adjust(adjust)))
}

#[test]
fn rotating_right_turns_the_picture_on_screen_and_waits_for_the_save() {
    let (_dir, mut harness, requests) = picture(vec![]);
    harness.send(Input::pointer_move(support::middle()));
    harness.advance(std::time::Duration::from_millis(300));
    let button = harness
        .centre("[aria-label=\"Rotate Right\"]")
        .expect("the rotate button");
    harness.send(Input::click(button));
    settle(&mut harness);
    assert!(asked(&requests).is_empty(), "nothing is written yet");
    assert_eq!(
        harness.count(".ds-titlebar-edited"),
        1,
        "the title bar says the picture has changes"
    );
    // The picture is shown turned a quarter clockwise: what was at the bottom left is at the top left.
    let image = harness.render().unwrap();
    for (name, dx, dy, want) in [
        ("top left", -12.0, -8.0, BLUE),
        ("top right", 12.0, -8.0, RED),
        ("bottom left", -12.0, 8.0, YELLOW),
        ("bottom right", 12.0, 8.0, GREEN),
    ] {
        let got = rgb(&image, dx, dy);
        assert!(is(got, want), "{name}: {got:?} is not {want:?}");
    }
    save(&mut harness);
    assert_eq!(
        asked(&requests),
        [write_of(Adjust::NONE.turned(QuarterTurn::Quarter))]
    );
}

#[test]
fn a_flip_is_shown_at_once_and_control_z_takes_it_back_before_the_file_is_asked() {
    let (_dir, mut harness, requests) = picture(vec![]);
    from_the_palette(&mut harness, "flip horizontal");
    let image = harness.render().unwrap();
    for (name, dx, dy, want) in [
        ("top left", -12.0, -8.0, GREEN),
        ("top right", 12.0, -8.0, RED),
        ("bottom left", -12.0, 8.0, YELLOW),
        ("bottom right", 12.0, 8.0, BLUE),
    ] {
        let got = rgb(&image, dx, dy);
        assert!(is(got, want), "{name}: {got:?} is not {want:?}");
    }
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('z')));
    settle(&mut harness);
    assert_eq!(harness.count(".ds-titlebar-edited"), 0, "taken back");
    assert!(
        asked(&requests).is_empty(),
        "the step was the window's, not the file's"
    );
    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('z'),
    ));
    settle(&mut harness);
    assert_eq!(harness.count(".ds-titlebar-edited"), 1, "done again");
    save(&mut harness);
    assert_eq!(
        asked(&requests),
        [write_of(Adjust::NONE.flipped(Axis::Horizontal))]
    );
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
        assert!(asked(&requests).is_empty(), "{name}: waits for the save");
        save(&mut harness);
        let Edit::Flip(axis) = *edit else {
            panic!("{name}: a flip")
        };
        assert_eq!(
            asked(&requests),
            [write_of(Adjust::NONE.flipped(axis))],
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
    // The expected dates are in UTC, whatever zone the machine is in.
    anyview_core::LocalZone::pin(0);
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
        listed.find("21 Sep 2026 at 14:13").unwrap() < listed.find("28 May 2026 at 20:26").unwrap(),
        "newest first: {listed}"
    );
    assert!(!listed.contains("UTC"), "no zone is named: {listed}");
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

#[test]
fn an_edit_that_loses_something_asks_first_and_cancel_leaves_the_file_alone() {
    let (_dir, paths) = folder(&[("anyview-image", "anim.webp", "anim.webp")]);
    let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), Wiring::default());
    settle(&mut harness);
    from_the_palette(&mut harness, "rotate right");
    assert!(
        harness.centre(".ds-alert").is_none(),
        "turning only changes what is shown"
    );
    save(&mut harness);
    assert!(harness.centre(".ds-alert").is_some(), "the alert is up");
    assert!(
        asked(&requests).is_empty(),
        "nothing is written before the answer"
    );
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert!(harness.centre(".ds-alert").is_none(), "Cancel puts it away");
    assert!(
        asked(&requests).is_empty(),
        "Cancel leaves the file untouched"
    );
    save(&mut harness);
    assert!(harness.centre(".ds-alert").is_some(), "it asks again");
    harness.send(Input::key(ShortcutKey::Enter));
    settle(&mut harness);
    assert_eq!(
        asked(&requests),
        [write_of(Adjust::NONE.turned(QuarterTurn::Quarter))]
    );
}

const SCALES: [u16; 2] = [100, 200];

/// The picture (48 by 32 pixels) in a window at `scale` percent.
fn picture_at(scale: u16) -> (tempfile::TempDir, Harness, Requests) {
    let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let wiring = Wiring {
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        ..Wiring::default()
    };
    let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    (dir, harness, requests)
}

fn at(x: f32, y: f32) -> Point {
    Point { x: Px(x), y: Px(y) }
}

fn cut(left: u32, top: u32, width: u32, height: u32) -> Adjust {
    Adjust {
        crop: Some(PixelRect {
            left: PixelLen(left),
            top: PixelLen(top),
            size: PixelSize {
                width: PixelLen(width),
                height: PixelLen(height),
            },
        }),
        ..Adjust::NONE
    }
}

#[test]
fn the_crop_tool_shows_a_rectangle_with_eight_handles_that_enter_cuts_to_and_escape_drops() {
    for scale in SCALES {
        let (_dir, mut harness, requests) = picture_at(scale);
        assert_eq!(
            harness.count(".viewer-crop"),
            0,
            "{scale}: no rectangle yet"
        );
        harness.send(Input::key(ShortcutKey::Char('c')));
        settle(&mut harness);
        assert_eq!(
            harness.count(".viewer-crop"),
            1,
            "{scale}: the tool draws it"
        );
        assert_eq!(
            harness.count(".viewer-crop-handle"),
            8,
            "{scale}: eight handles"
        );
        assert_eq!(
            harness.count(".viewer-crop-guide"),
            4,
            "{scale}: the thirds are drawn (and shown while a grip is held)"
        );
        // Drag the right edge in by 16 pixels of the picture.
        let half = 24.0 / (f32::from(scale) / 100.0);
        let edge = at(450.0 + half, 300.0);
        let dragged = at(450.0 + half - 16.0 / (f32::from(scale) / 100.0), 300.0);
        harness.send(Input::drag(edge, dragged, 4));
        settle(&mut harness);
        assert_eq!(
            harness.count(".viewer-crop"),
            1,
            "{scale}: still up after the drag"
        );
        harness.send(Input::key(ShortcutKey::Enter));
        settle(&mut harness);
        assert_eq!(
            harness.count(".viewer-crop"),
            0,
            "{scale}: Enter puts it away"
        );
        assert_eq!(
            harness.count(".ds-titlebar-edited"),
            1,
            "{scale}: the cut is a change"
        );
        assert!(
            asked(&requests).is_empty(),
            "{scale}: nothing is written yet"
        );
        save(&mut harness);
        assert_eq!(asked(&requests), [write_of(cut(0, 0, 32, 32))], "{scale}");
    }
}

#[test]
fn escape_puts_the_crop_rectangle_away_and_leaves_the_picture_as_it_was() {
    let (_dir, mut harness, requests) = picture(vec![]);
    harness.send(Input::key(ShortcutKey::Char('c')));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-crop"), 1);
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-crop"), 0);
    assert_eq!(harness.count(".ds-titlebar-edited"), 0);
    assert_eq!(
        harness.attr(".viewer-raster", "data-pan").as_deref(),
        Some("on"),
        "back on the hand"
    );
    assert!(asked(&requests).is_empty());
}

#[test]
fn the_title_bar_offers_crop_beside_select_and_pan_for_a_picture_that_can_be_saved() {
    let (_dir, harness, _) = picture(vec![]);
    assert_eq!(
        harness.count(".ds-titlebar-trailing .ds-segmented-segment"),
        3,
        "Select, Pan and Crop"
    );
}

#[test]
fn adjust_size_opens_a_dialog_on_the_size_the_picture_has_and_cancel_changes_nothing() {
    let (_dir, mut harness, requests) = picture(vec![]);
    from_the_palette(&mut harness, "adjust size");
    assert_eq!(harness.count(".viewer-resize"), 1, "the dialog is up");
    let said = harness.text_of(".viewer-resize").unwrap_or_default();
    assert!(said.contains("48 \u{d7} 32 pixels"), "{said}");
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-resize"), 0);
    assert_eq!(harness.count(".ds-titlebar-edited"), 0);
    assert!(asked(&requests).is_empty());
}

#[test]
fn closing_a_picture_with_changes_asks_and_cancel_keeps_the_window() {
    let (_dir, mut harness, requests) = picture(vec![]);
    from_the_palette(&mut harness, "rotate right");
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('w')));
    settle(&mut harness);
    assert!(harness.centre(".ds-alert").is_some(), "the question is up");
    let text = harness.text_of(".ds-alert").unwrap_or_default();
    assert!(text.contains("save the changes"), "{text}");
    let closes = |requests: &Requests| {
        requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| matches!(request, HostRequest::CloseWindow))
    };
    assert!(!closes(&requests));
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert!(harness.centre(".ds-alert").is_none(), "Cancel puts it away");
    assert!(!closes(&requests), "and the window stays");
    assert_eq!(harness.count(".ds-titlebar-edited"), 1, "with its changes");
    // Return is Save: the file is asked to be written, and the window waits for it.
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('w')));
    settle(&mut harness);
    harness.send(Input::key(ShortcutKey::Enter));
    settle(&mut harness);
    assert_eq!(
        asked(&requests),
        [write_of(Adjust::NONE.turned(QuarterTurn::Quarter))]
    );
    assert!(
        !closes(&requests),
        "it closes once the saved file is read again"
    );
}

#[test]
fn a_picture_that_cannot_be_written_has_nothing_to_edit_and_the_save_chord_offers_export() {
    let (_dir, paths) = folder(&[("anyview-image", "spin.gif", "spin.gif")]);
    let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), Wiring::default());
    settle(&mut harness);
    assert_eq!(
        harness.count(".ds-titlebar-trailing .ds-segmented-segment"),
        2,
        "no Crop where the file cannot be saved with changes"
    );
    save(&mut harness);
    assert_eq!(
        harness.count(".viewer-export"),
        1,
        "the Export dialog is offered"
    );
    assert!(asked(&requests).is_empty());
}
