//! The text stage under the harness: the keys scroll it, find marks its hits and steps through
//! them, and long lines wrap to the window's width.

#![allow(clippy::unwrap_used)]

mod support;

use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::{VIEW, shot, window};

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

#[test]
fn find_marks_the_hits_steps_through_them_and_closes_with_escape() {
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
        harness.count(".viewer-find"),
        0,
        "no bar until it is asked for"
    );
    chord(&mut harness, 'f');
    assert_eq!(harness.count(".viewer-find"), 1, "command F brings the bar");
    type_text(&mut harness, "needle");
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("1 of 6"),
        "six rows have it, and the first is the nearest"
    );
    assert!(harness.count(".viewer-hit") >= 1, "the hit is marked");
    assert_eq!(
        harness.count(".viewer-hit-current"),
        1,
        "and one is the current"
    );
    let first = first_line(&harness).unwrap();
    assert!(first <= 50, "the first hit is on screen from row {first}");
    save(&mut harness, "find.png");
    harness.send(Input::key(ShortcutKey::Enter));
    settle(&mut harness);
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("2 of 6")
    );
    let second = first_line(&harness).unwrap();
    assert!(
        second > first,
        "the view followed the hit: {first} then {second}"
    );
    harness.send(Input::chord(&[ShortcutKey::Shift], ShortcutKey::Enter));
    settle(&mut harness);
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("1 of 6"),
        "shift and return step back"
    );
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-find"), 0, "Esc closes the bar");
    assert_eq!(harness.count(".viewer-hit"), 0, "and takes the marks away");
}

#[test]
fn a_find_with_no_match_says_so() {
    let (_dir, mut harness) = open(&numbered(40));
    chord(&mut harness, 'f');
    type_text(&mut harness, "zzz");
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("No matches")
    );
    assert_eq!(harness.count(".viewer-hit"), 0);
}

#[test]
fn typing_in_the_find_bar_does_not_run_the_windows_keys() {
    // `w` toggles wrapping and `v` the source view, and `0` zooms: none of them while typing.
    let (_dir, mut harness) = open(&numbered(40));
    chord(&mut harness, 'f');
    type_text(&mut harness, "wv0");
    assert_eq!(
        harness.attr(".viewer-line", "data-wrap").as_deref(),
        Some("on"),
        "w did not toggle wrapping"
    );
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
        wrapped > 3.0 * 18.0,
        "600 characters take several rows in a 900 px window: {wrapped}"
    );
    press(&mut harness, ShortcutKey::Char('w'));
    assert_eq!(
        harness.attr(".viewer-line", "data-wrap").as_deref(),
        Some("off")
    );
    let single = row(&harness);
    assert!(
        (single - 18.0).abs() < 1.0,
        "with wrap off the line is one row: {single}"
    );
}
