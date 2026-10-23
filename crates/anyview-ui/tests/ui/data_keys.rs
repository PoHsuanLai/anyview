//! The keys of a table and a JSON tree under the harness: the arrow keys, Page Up and Down, Home
//! and End move through its rows, and they never carry the reader off to another file; Esc puts
//! the cursor away, and then Left and Right walk the folder again. Find opens the nodes above a hit
//! of a tree, and zoom scales its rows.

use crate::support;

use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::Harness;
use std::path::PathBuf;
use support::{chord, press, settle, title, type_text, window};

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

/// The selected row's text: a table's held columns and the ones that scroll are each a row, and
/// the one that scrolls is the selected row assistive technology hears.
fn selected_row(harness: &Harness) -> Option<String> {
    let held = harness.text_of(".ds-vtable-held .ds-vtable-item[data-selected=yes]");
    let rest = harness
        .text_of(".ds-vtable-rows [aria-selected=true]")
        .or_else(|| harness.text_of(".viewer-stage .ds-row[aria-selected=true]"));
    match (held, rest) {
        (Some(held), Some(rest)) => Some(format!("{held} {rest}")),
        (held, rest) => rest.or(held),
    }
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

const DEEP: &str = r#"{"a":{"b":{"c":"the needle"}},"z":[1,2]}"#;

#[test]
fn find_in_a_tree_opens_the_nodes_above_a_hit_and_marks_it() {
    let (_dir, paths) = folder_of(&[("deep.json", DEEP.to_owned())]);
    let mut harness = window_on(&paths);
    let nodes = |harness: &Harness| harness.count(".viewer-node");
    assert_eq!(
        nodes(&harness),
        3,
        "the root, a and z: only the top level is open"
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
        listed.contains("In This File") && listed.contains("c: \"the needle\""),
        "the hit is listed by its key and its value: {listed}"
    );
    assert_eq!(
        nodes(&harness),
        5,
        "a, b and c are open now: root, a, b, c, z"
    );
    assert!(
        selected_row(&harness).is_some_and(|row| row.contains("needle")),
        "the cursor is on the hit: {:?}",
        selected_row(&harness)
    );
    assert!(harness.count(".viewer-hit") >= 1, "the words are marked");
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(harness.count(".viewer-hit"), 0, "Esc puts the marks away");
}

#[test]
fn a_find_in_a_tree_matches_keys_as_well_as_values() {
    let (_dir, paths) = folder_of(&[("deep.json", DEEP.to_owned())]);
    let mut harness = window_on(&paths);
    chord(&mut harness, 'f');
    type_text(&mut harness, "z");
    assert!(
        selected_row(&harness).is_some_and(|row| row.starts_with('z')),
        "the key z is the hit: {:?}",
        selected_row(&harness)
    );
}

#[test]
fn zoom_scales_the_rows_of_a_tree_and_actual_size_returns() {
    let row_height = |harness: &Harness| {
        harness
            .rect(".viewer-stage .ds-list-item")
            .expect("a row")
            .size
            .height
            .0
    };
    let (_dir, paths) = folder_of(&[("deep.json", DEEP.to_owned())]);
    let base = row_height(&window_on(&paths));
    // name, keys pressed, the row height as a share of the actual one
    const CASES: &[(&str, &[char], f32)] = &[
        ("in", &['+'], 1.25),
        ("in past the top stops at three times", &['+'; 9], 3.0),
        ("out", &['-'], 0.75),
        ("in and back to actual size", &['+', '+', '0'], 1.0),
    ];
    for (name, keys, share) in CASES {
        let mut harness = window_on(&paths);
        for key in *keys {
            press(&mut harness, ShortcutKey::Char(*key));
        }
        let got = row_height(&harness);
        assert!(
            (got - base * share).abs() < 1.0,
            "{name}: a row is {got} px, wanted {share} of {base}"
        );
    }
}
