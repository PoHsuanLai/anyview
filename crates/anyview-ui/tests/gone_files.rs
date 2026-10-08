//! A file that leaves the folder while the window is open: the one the person just moved to the
//! Trash, and one deleted from outside. The window moves on to a neighbour that is still there,
//! as a mature viewer does, and the arrows never land on the missing file.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FilePath;
use anyview_ui::HostRequest;
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use support::{Requests, Wiring, press, settle, text_file, title, wired};

const SCALES: [u16; 2] = [100, 200];

fn three(
    scale: u16,
) -> (
    tempfile::TempDir,
    Vec<PathBuf>,
    Harness,
    Requests,
    anyview_ui::Edge,
) {
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<PathBuf> = (1..=3)
        .map(|n| text_file(dir.path(), &format!("f{n}.txt"), &format!("w{n}"), 5))
        .collect();
    let wiring = Wiring {
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        ..Wiring::default()
    };
    let (mut harness, requests, edge) = wired(&files, 1, Appearance::default(), wiring);
    settle(&mut harness);
    (dir, files, harness, requests, edge)
}

fn trashes(requests: &Requests) -> usize {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| matches!(request, HostRequest::Trash))
        .count()
}

/// Press the sheet's Move to Trash button; Return is Cancel there, as the destructive button is never
/// the default.
fn confirm_trash(harness: &mut Harness) {
    let button = harness
        .centre(".ds-alert-footer .ds-alert-slot:last-child .ds-button")
        .expect("the Move to Trash button");
    harness.send(Input::click(button));
    settle(harness);
}

fn ask_to_trash(harness: &mut Harness) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Backspace));
    settle(harness);
}

#[test]
fn cancelling_the_trash_sheet_asks_the_host_for_nothing_and_keeps_the_file() {
    for scale in SCALES {
        let (_dir, _files, mut harness, requests, _) = three(scale);
        ask_to_trash(&mut harness);
        assert!(
            harness.centre(".ds-alert").is_some(),
            "{scale}: the sheet is up"
        );
        harness.send(Input::key(ShortcutKey::Escape));
        settle(&mut harness);
        assert!(
            harness.centre(".ds-alert").is_none(),
            "{scale}: Esc puts it away"
        );
        assert_eq!(trashes(&requests), 0, "{scale}");
        assert_eq!(title(&harness).as_deref(), Some("f2.txt"), "{scale}");
    }
}

#[test]
fn the_move_to_trash_button_asks_the_host_once() {
    for scale in SCALES {
        let (_dir, _files, mut harness, requests, _) = three(scale);
        ask_to_trash(&mut harness);
        let button = harness
            .centre(".ds-alert-footer .ds-alert-slot:last-child .ds-button")
            .expect("the Move to Trash button");
        harness.send(Input::click(button));
        harness.send(Input::click(button));
        settle(&mut harness);
        assert_eq!(
            trashes(&requests),
            1,
            "{scale}: a double click trashes once"
        );
    }
}

#[test]
fn after_the_trash_the_window_shows_the_next_file_not_a_moved_or_deleted_page() {
    for scale in SCALES {
        let (_dir, files, mut harness, _, edge) = three(scale);
        ask_to_trash(&mut harness);
        confirm_trash(&mut harness);
        // What the host and its watcher do once the file is in the Trash.
        std::fs::remove_file(&files[1]).unwrap();
        edge.changed(FilePath::new(&files[1]).unwrap());
        settle(&mut harness);
        settle(&mut harness);
        assert_eq!(
            harness.count(".viewer-failed"),
            0,
            "{scale}: the person trashed f2.txt on purpose; the window shows {:?}",
            harness.text_of(".viewer-failed")
        );
        assert_ne!(
            title(&harness).as_deref(),
            Some("f2.txt"),
            "{scale}: moved on from the trashed file"
        );
    }
}

#[test]
fn an_arrow_key_skips_a_file_deleted_from_outside_the_window() {
    for scale in SCALES {
        let (_dir, files, mut harness, _, _) = three(scale);
        press(&mut harness, ShortcutKey::Left);
        assert_eq!(title(&harness).as_deref(), Some("f1.txt"), "{scale}");
        std::fs::remove_file(&files[1]).unwrap();
        press(&mut harness, ShortcutKey::Right);
        assert_eq!(
            title(&harness).as_deref(),
            Some("f3.txt"),
            "{scale}: f2 is gone, the arrow goes to f3; shows {:?}",
            harness.text_of(".viewer-failed")
        );
    }
}
