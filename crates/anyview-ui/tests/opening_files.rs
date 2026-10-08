//! Opening a file: Open… from the palette, the menu and ⌘O, a chosen or dropped list, and what a
//! drop does to whatever is already open on the window.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FilePath;
use anyview_ui::HostRequest;
use ds::file_drop::drag::{FileDragInput, Offer};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_core::press::PointerButton;
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use support::{Requests, Wiring, middle, press, settle, text_file, title, wired};

const SCALES: [u16; 2] = [100, 200];

fn viewport(scale: u16) -> Wiring {
    Wiring {
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        ..Wiring::default()
    }
}

fn folder_of(names: &[&str]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let files = names
        .iter()
        .map(|name| text_file(dir.path(), name, name, 5))
        .collect();
    (dir, files)
}

fn picks(requests: &Requests) -> usize {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| matches!(request, HostRequest::PickFile))
        .count()
}

fn drop_files(harness: &mut Harness, files: Vec<PathBuf>) {
    for step in [
        FileDragInput::Entered {
            point: Some(middle()),
        },
        FileDragInput::Offered(Offer::Files(files)),
        FileDragInput::Moved { point: middle() },
        FileDragInput::Dropped,
    ] {
        harness.send(Input::FileDrag(step));
    }
    settle(harness);
    settle(harness);
}

fn palette(harness: &mut Harness, words: &str) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    for letter in words.chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(harness);
}

#[test]
fn command_o_the_palette_and_the_menu_each_ask_the_host_for_one_chooser() {
    for scale in SCALES {
        let (_dir, files) = folder_of(&["a.txt", "b.txt"]);
        let (mut harness, requests, edge) =
            wired(&files, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('o')));
        settle(&mut harness);
        assert_eq!(picks(&requests), 1, "{scale}: command-O");
        // The chooser is cancelled; only then is another asked for.
        edge.chosen(Vec::new());
        settle(&mut harness);
        palette(&mut harness, "open");
        harness.send(Input::key(ShortcutKey::Enter));
        settle(&mut harness);
        assert_eq!(picks(&requests), 2, "{scale}: the palette's Open…");
        let at = Point {
            x: Px(300.0),
            y: Px(200.0),
        };
        harness.send(Input::press(at, PointerButton::Secondary));
        settle(&mut harness);
        let row = harness.html().find("Open\u{2026}").is_some();
        assert!(row, "{scale}: the menu has Open…");
    }
}

#[test]
fn command_o_twice_while_the_chooser_is_pending_asks_for_one_chooser() {
    for scale in SCALES {
        let (_dir, files) = folder_of(&["a.txt", "b.txt"]);
        let (mut harness, requests, _) = wired(&files, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        for _ in 0..2 {
            harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('o')));
            settle(&mut harness);
        }
        assert_eq!(
            picks(&requests),
            1,
            "{scale}: the chooser has not answered, a second command-O must not open a second one"
        );
    }
}

#[test]
fn a_chosen_list_is_the_list_the_arrows_walk_not_the_folder() {
    for scale in SCALES {
        let (_dir, files) = folder_of(&["a.txt", "b.txt", "c.txt", "d.txt"]);
        let (mut harness, _, edge) = wired(&files, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        edge.chosen(vec![
            FilePath::new(&files[3]).unwrap(),
            FilePath::new(&files[1]).unwrap(),
        ]);
        settle(&mut harness);
        settle(&mut harness);
        assert_eq!(title(&harness).as_deref(), Some("d.txt"), "{scale}");
        press(&mut harness, ShortcutKey::Right);
        assert_eq!(
            title(&harness).as_deref(),
            Some("b.txt"),
            "{scale}: the second of the chosen"
        );
        press(&mut harness, ShortcutKey::Right);
        assert_eq!(
            title(&harness).as_deref(),
            Some("b.txt"),
            "{scale}: and the end of the list"
        );
    }
}

#[test]
fn a_drop_while_a_rename_sheet_is_up_does_not_leave_the_sheet_over_the_new_file() {
    for scale in SCALES {
        let (_dir, files) = folder_of(&["a.txt", "b.txt"]);
        let (mut harness, _, _) = wired(&files, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        palette(&mut harness, "rename");
        harness.send(Input::key(ShortcutKey::Enter));
        settle(&mut harness);
        assert_eq!(
            harness.count(".viewer-sheet"),
            1,
            "{scale}: the sheet is up"
        );
        drop_files(&mut harness, vec![files[1].clone()]);
        assert_eq!(title(&harness).as_deref(), Some("b.txt"), "{scale}");
        assert_eq!(
            harness.count(".viewer-sheet"),
            0,
            "{scale}: a name typed for a.txt must not be applied to b.txt"
        );
    }
}

#[test]
fn the_menus_open_row_asks_the_host_for_a_chooser_at_both_scales() {
    for scale in SCALES {
        let (_dir, files) = folder_of(&["a.txt", "b.txt"]);
        let (mut harness, requests, _) = wired(&files, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        let at = Point {
            x: Px(300.0),
            y: Px(200.0),
        };
        harness.send(Input::press(at, PointerButton::Secondary));
        settle(&mut harness);
        let row = (1..=40)
            .find(|n| {
                harness
                    .text_of(&format!(".ds-menu-item:nth-child({n})"))
                    .is_some_and(|text| text.trim() == "Open\u{2026}")
            })
            .expect("an Open… row");
        let centre = harness
            .centre(&format!(".ds-menu-item:nth-child({row})"))
            .unwrap();
        harness.send(Input::click(centre));
        harness.advance(std::time::Duration::from_secs(1));
        assert_eq!(picks(&requests), 1, "{scale}");
        assert_eq!(harness.count(".ds-menu"), 0, "{scale}");
    }
}

#[test]
fn a_drop_while_the_export_sheet_is_up_does_not_leave_it_over_the_new_file() {
    for scale in SCALES {
        let (_dir, files) = folder_of(&["a.txt", "b.txt"]);
        let (mut harness, _, _) = wired(&files, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        palette(&mut harness, "export");
        harness.send(Input::key(ShortcutKey::Enter));
        settle(&mut harness);
        assert_eq!(harness.count(".viewer-sheet"), 1, "{scale}");
        drop_files(&mut harness, vec![files[1].clone()]);
        assert_eq!(title(&harness).as_deref(), Some("b.txt"), "{scale}");
        assert_eq!(harness.count(".viewer-sheet"), 0, "{scale}");
    }
}
