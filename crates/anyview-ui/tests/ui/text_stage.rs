//! The text stage under the harness: the keys scroll it, find (in the palette) marks its hits and steps through
//! them, and long lines wrap to the window's width.

use crate::support;

use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::{VIEW, Wiring, shot, window, wired};

fn middle() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

fn file(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    std::fs::canonicalize(path).unwrap()
}

fn numbered(lines: u32) -> String {
    (1..=lines)
        .map(|n| format!("row {n} of the file\n"))
        .collect()
}

fn settle(harness: &mut Harness) {
    harness.advance(Duration::from_millis(300));
}

fn press(harness: &mut Harness, key: ShortcutKey) {
    harness.send(Input::key(key));
    settle(harness);
}

fn chord(harness: &mut Harness, key: char) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char(key)));
    settle(harness);
}

fn type_text(harness: &mut Harness, text: &str) {
    for c in text.chars() {
        harness.send(Input::key(ShortcutKey::Char(c)));
    }
    settle(harness);
}

fn save(harness: &mut Harness, name: &str) {
    if let Some(path) = shot(name) {
        harness.render().unwrap().save(path).unwrap();
    }
}

fn first_line(harness: &Harness) -> Option<u32> {
    harness.text_of(".viewer-lineno")?.trim().parse().ok()
}

fn open(body: &str) -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "notes.txt", body);
    let (mut harness, _) = window(&[path], 0, Appearance::default());
    settle(&mut harness);
    harness.send(Input::pointer_move(middle()));
    settle(&mut harness);
    (dir, harness)
}

#[test]
fn the_keys_scroll_a_text_by_line_by_page_and_to_its_ends() {
    let (_dir, mut harness) = open(&numbered(500));
    assert_eq!(first_line(&harness), Some(1));
    press(&mut harness, ShortcutKey::Down);
    assert_eq!(first_line(&harness), Some(2), "a line down");
    press(&mut harness, ShortcutKey::Up);
    assert_eq!(first_line(&harness), Some(1), "a line up");
    press(&mut harness, ShortcutKey::PageDown);
    let page = first_line(&harness).unwrap();
    assert!(
        page > 10 && page < 60,
        "a page is what fits the window: {page}"
    );
    press(&mut harness, ShortcutKey::PageDown);
    assert_eq!(
        first_line(&harness),
        Some(page + (page - 1)),
        "twice a page"
    );
    press(&mut harness, ShortcutKey::PageUp);
    assert_eq!(first_line(&harness), Some(page));
    press(&mut harness, ShortcutKey::End);
    let end = first_line(&harness).unwrap();
    assert!(
        end > 440 && end < 500,
        "the last page is at the bottom: {end}"
    );
    assert!(
        harness
            .text_of(".viewer-text")
            .unwrap()
            .contains("row 500 of the file"),
        "and the last line is on it"
    );
    press(&mut harness, ShortcutKey::Home);
    assert_eq!(first_line(&harness), Some(1));
}

fn capsule(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

#[test]
fn find_lives_in_the_palette_marks_the_hits_behind_it_and_enter_jumps_and_leaves_them_to_step() {
    let body: String = (1..=300)
        .map(|n| {
            if n % 50 == 0 {
                format!("row {n} has the needle in it\n")
            } else {
                format!("row {n} is hay\n")
            }
        })
        .collect();
    let (_dir, mut harness) = open(&body);
    assert_eq!(
        harness.count(".ds-palette"),
        0,
        "nothing until it is asked for"
    );
    chord(&mut harness, 'f');
    assert_eq!(
        harness.count(".ds-palette"),
        1,
        "command F brings the palette"
    );
    type_text(&mut harness, "needle");
    let listed = harness.text_of(".ds-palette").unwrap_or_default();
    assert!(
        listed.contains("In This File")
            && listed.contains("Line 50")
            && listed.contains("Line 300"),
        "the hits are listed with their lines, before the commands: {listed}"
    );
    assert!(
        harness.count(".viewer-hit") >= 1,
        "the hit is marked behind the palette"
    );
    assert_eq!(
        harness.count(".viewer-hit-current"),
        1,
        "and one is the current"
    );
    let first = first_line(&harness).unwrap();
    assert!(first <= 50, "the first hit is on screen from row {first}");
    save(&mut harness, "find.png");
    press(&mut harness, ShortcutKey::Down);
    let second = first_line(&harness).unwrap();
    assert!(
        second > first,
        "moving the highlight to the next hit moved the view: {first} then {second}"
    );
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(harness.count(".ds-palette"), 0, "Enter jumps and closes");
    assert!(harness.count(".viewer-hit") >= 1, "the hits stay marked");
    assert!(
        capsule(&harness).contains("2 of 6"),
        "the capsule says where the reader is: {}",
        capsule(&harness)
    );
    chord(&mut harness, 'g');
    assert!(
        capsule(&harness).contains("3 of 6"),
        "command G steps to the next: {}",
        capsule(&harness)
    );
    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('g'),
    ));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("2 of 6"),
        "and shift command G steps back: {}",
        capsule(&harness)
    );
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(harness.count(".viewer-hit"), 0, "Esc puts the find away");
}

#[test]
fn escape_in_the_find_palette_clears_the_marks_and_a_find_with_no_match_says_so() {
    let (_dir, mut harness) = open(&numbered(40));
    chord(&mut harness, 'f');
    // `w` toggles wrapping and `v` the source view, and `0` zooms: none of them while typing.
    type_text(&mut harness, "wv0");
    assert!(
        harness
            .text_of(".ds-palette")
            .unwrap_or_default()
            .contains("No matches"),
        "nothing in the file is named so"
    );
    assert_eq!(harness.count(".viewer-hit"), 0);
    assert_eq!(
        harness.attr(".viewer-line", "data-wrap").as_deref(),
        Some("on"),
        "w did not toggle wrapping"
    );
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(harness.count(".ds-palette"), 0, "Esc closes it");
    chord(&mut harness, 'f');
    type_text(&mut harness, "row 7");
    assert!(harness.count(".viewer-hit") >= 1, "a phrase is found");
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(harness.count(".viewer-hit"), 0, "and Esc clears its marks");
}

#[test]
fn a_long_line_wraps_to_the_window_when_wrap_is_on_and_runs_on_when_it_is_off() {
    let long = "word ".repeat(120);
    let (_dir, mut harness) = open(&format!("{long}\nshort\n"));
    let row = |harness: &Harness| harness.rect(".viewer-line").unwrap().size.height.0;
    assert_eq!(
        harness.attr(".viewer-line", "data-wrap").as_deref(),
        Some("on")
    );
    let wrapped = row(&harness);
    save(&mut harness, "wrapped.png");
    assert!(
        wrapped > 3.0 * 20.0,
        "600 characters take several rows in a 900 px window: {wrapped}"
    );
    press(&mut harness, ShortcutKey::Char('w'));
    assert_eq!(
        harness.attr(".viewer-line", "data-wrap").as_deref(),
        Some("off")
    );
    let single = row(&harness);
    assert!(
        (single - 20.0).abs() < 1.0,
        "with wrap off the line is one row: {single}"
    );
}

/// The bytes of the save the window asked of its host last, if it asked.
fn saved_bytes(requests: &support::Requests) -> Option<Vec<u8>> {
    requests.lock().unwrap().iter().rev().find_map(|request| {
        if let anyview_ui::HostRequest::SaveText(save) = request {
            Some(save.clone().into_bytes())
        } else {
            None
        }
    })
}

#[test]
fn a_text_is_edited_in_place_saved_and_asked_about_before_the_window_closes() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "notes.txt", "hello\nworld\n");
    let (mut harness, requests, edge) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    harness.send(Input::pointer_move(middle()));
    settle(&mut harness);
    // Return edits; the text is read and the surface has the keyboard.
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(
        harness.count(".ds-edit"),
        1,
        "the lines are an edit surface now"
    );
    assert_eq!(
        harness.count(".ds-titlebar-edited"),
        0,
        "nothing is changed yet"
    );
    type_text(&mut harness, "x");
    assert_eq!(
        harness.count(".ds-titlebar-edited"),
        1,
        "the title bar's dot says there are unsaved changes"
    );
    assert_eq!(
        harness.text_of(".viewer-code").as_deref(),
        Some("xhello"),
        "typed at the caret"
    );
    chord(&mut harness, 's');
    assert_eq!(
        saved_bytes(&requests).as_deref(),
        Some(&b"xhello\nworld\n"[..]),
        "command S asks the host to write the text, line endings and all"
    );
    // The host wrote it and says so: the dot goes.
    edge.saved(
        anyview_core::FilePath::new(&path).unwrap(),
        anyview_ui::SaveEnd::Written,
    );
    settle(&mut harness);
    assert_eq!(harness.count(".ds-titlebar-edited"), 0, "saved");
    // Another change, and the window will not close on it without asking.
    type_text(&mut harness, "y");
    assert_eq!(harness.count(".ds-titlebar-edited"), 1);
    chord(&mut harness, 'w');
    assert!(
        harness.html().contains("Save changes to"),
        "closing asks what to do with the changes"
    );
    press(&mut harness, ShortcutKey::Escape);
    assert!(
        !harness.html().contains("Save changes to"),
        "Cancel keeps the person where they were"
    );
    assert_eq!(harness.count(".ds-edit"), 1, "still editing");
}
