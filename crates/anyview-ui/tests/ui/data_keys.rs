//! The keys of a table and a JSON tree under the harness: the arrow keys, Page Up and Down, Home
//! and End move through its rows, and they never carry the reader off to another file; Esc puts
//! the cursor away, and then Left and Right walk the folder again.

use crate::support;

use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Harness, Query};
use std::path::PathBuf;
use support::{press, settle, title, window};

fn rows(count: u32) -> String {
    let mut text = String::from("id,name\n");
    for n in 1..=count {
        text.push_str(&format!("{n},row {n}\n"));
    }
    text
}

fn folder_of(tables: &[(&str, String)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let paths = tables
        .iter()
        .map(|(name, body)| {
            let path = dir.path().join(name);
            std::fs::write(&path, body).unwrap();
            std::fs::canonicalize(path).unwrap()
        })
        .collect();
    (dir, paths)
}

fn window_on(paths: &[PathBuf]) -> Harness {
    let (mut harness, _) = window(paths, 0, Appearance::default());
    settle(&mut harness);
    harness
}

fn selected_row(harness: &Harness) -> Option<String> {
    harness.text_of(".ds-row[aria-selected=true]")
}

#[test]
fn the_keys_move_through_a_tables_rows_and_do_not_leave_the_file() {
    let (_dir, paths) = folder_of(&[("a.csv", rows(200)), ("b.csv", rows(3))]);
    let mut harness = window_on(&paths);
    assert_eq!(title(&harness).as_deref(), Some("a.csv"));
    for (key, after) in [
        (ShortcutKey::Down, "row 1"),
        (ShortcutKey::Down, "row 2"),
        (ShortcutKey::Up, "row 1"),
        (ShortcutKey::End, "row 200"),
        (ShortcutKey::Home, "row 1"),
    ] {
        press(&mut harness, key);
        assert!(
            selected_row(&harness).is_some_and(|row| row.contains(after)),
            "{key:?} lands on {after}: {:?}",
            selected_row(&harness)
        );
        assert_eq!(
            title(&harness).as_deref(),
            Some("a.csv"),
            "{key:?} stays in the file"
        );
    }
    press(&mut harness, ShortcutKey::PageDown);
    let after_page = selected_row(&harness).unwrap_or_default();
    assert!(
        !after_page.contains("row 1 "),
        "a page moves further than a row: {after_page}"
    );
}

#[test]
fn right_stays_put_while_a_row_is_picked_and_walks_the_folder_after_escape() {
    let (_dir, paths) = folder_of(&[("a.csv", rows(20)), ("b.csv", rows(3))]);
    let mut harness = window_on(&paths);
    press(&mut harness, ShortcutKey::Down);
    press(&mut harness, ShortcutKey::Right);
    assert_eq!(
        title(&harness).as_deref(),
        Some("a.csv"),
        "a row is picked: Right is not a way to another file"
    );
    press(&mut harness, ShortcutKey::Escape);
    press(&mut harness, ShortcutKey::Right);
    assert_eq!(
        title(&harness).as_deref(),
        Some("b.csv"),
        "with no row picked it walks"
    );
}

#[test]
fn the_keys_move_through_a_json_trees_rows_too() {
    let body: String = format!(
        "[{}]",
        (0..100)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let (_dir, paths) = folder_of(&[("a.json", body), ("b.json", "[1]".to_owned())]);
    let mut harness = window_on(&paths);
    press(&mut harness, ShortcutKey::Down);
    assert!(selected_row(&harness).is_some(), "Down picks a row");
    press(&mut harness, ShortcutKey::End);
    assert!(
        selected_row(&harness).is_some_and(|row| row.contains("99")),
        "End picks the last: {:?}",
        selected_row(&harness)
    );
    assert_eq!(title(&harness).as_deref(), Some("a.json"));
}
