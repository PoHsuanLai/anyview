//! The table and tree stages under the harness: a very large table mounts only the rows near the
//! window, a tree shows its first level, and the headers come from the file.

use crate::support;

use anyview_ui::WorkKind;
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use std::time::Duration;
use support::{Gate, VIEW, Wiring, chord, press, type_text, window, wired};

fn settle(harness: &mut Harness) {
    harness.advance(Duration::from_millis(300));
}

fn open(name: &str, body: &str) -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join(name);
    std::fs::write(&path, body).unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let (mut harness, _) = window(&[path], 0, Appearance::default());
    settle(&mut harness);
    harness.send(Input::pointer_move(Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }));
    settle(&mut harness);
    (dir, harness)
}

#[test]
fn a_table_of_twenty_thousand_rows_mounts_only_a_window_of_them() {
    let mut body = String::from("id,name,score\n");
    for n in 0..20_000 {
        body.push_str(&format!("{n},name {n},{}\n", n % 97));
    }
    let (_dir, harness) = open("big.csv", &body);
    let mounted = harness.count(".viewer-row-number");
    assert!(mounted > 0, "the first rows are drawn");
    assert!(mounted < 200, "{mounted} rows are mounted for 20000");
    assert_eq!(harness.text_of(".ds-table-title").as_deref(), Some("#"));
}

#[test]
fn a_json_file_shows_as_a_tree_of_its_top_level() {
    let (_dir, harness) = open("tree.json", r#"{"name":"anyview","tags":["a","b"],"n":3}"#);
    assert!(harness.count(".viewer-node") >= 3, "the keys are rows");
    assert!(harness.text_of(".viewer-node-key").is_some());
}

/// The first column's header, which sorts the rows when pressed.
const HEAD: &str = ".ds-table-column[data-sorting=\"sortable\"]";

/// A table whose first column is numbers out of order, opened with the sorts held back.
fn held_sorts() -> (tempfile::TempDir, Harness, std::sync::Arc<Gate>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scores.csv");
    std::fs::write(&path, "n,name\n2,b\n3,c\n1,a\n").unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let gate = Gate::holding(&[WorkKind::Sort]);
    let wiring = Wiring {
        workers: Some(gate.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    (dir, harness, gate)
}

fn press_head(harness: &mut Harness) {
    let at = harness.centre(HEAD).expect("a sortable header");
    harness.send(Input::click(at));
    settle(harness);
}

fn first_row(harness: &Harness) -> String {
    harness.text_of(".ds-table-cells").unwrap_or_default()
}

#[test]
fn a_sort_shows_in_the_header_at_once_and_the_rows_swap_when_the_worker_answers() {
    let (_dir, mut harness, gate) = held_sorts();
    assert!(first_row(&harness).contains("2b"), "the file's own order");
    press_head(&mut harness);
    assert_eq!(
        harness.attr(HEAD, "aria-sort").as_deref(),
        Some("ascending")
    );
    assert_eq!(gate.held(WorkKind::Sort), 1, "the sort went to a worker");
    assert!(
        first_row(&harness).contains("2b"),
        "the rows wait in their order"
    );
    gate.release(WorkKind::Sort);
    settle(&mut harness);
    assert!(
        first_row(&harness).contains("1a"),
        "{}",
        first_row(&harness)
    );
}

#[test]
fn the_answer_to_a_sort_a_newer_press_replaced_is_dropped() {
    let (_dir, mut harness, gate) = held_sorts();
    press_head(&mut harness);
    press_head(&mut harness);
    assert_eq!(
        harness.attr(HEAD, "aria-sort").as_deref(),
        Some("descending")
    );
    assert_eq!(gate.held(WorkKind::Sort), 2);
    gate.release_one(WorkKind::Sort);
    settle(&mut harness);
    assert!(
        first_row(&harness).contains("2b"),
        "the ascending answer came too late: {}",
        first_row(&harness)
    );
    gate.release_one(WorkKind::Sort);
    settle(&mut harness);
    assert!(
        first_row(&harness).contains("3c"),
        "{}",
        first_row(&harness)
    );
}

/// A sheet of cities in which Lima is every hundredth row.
fn cities() -> String {
    let mut body = String::from("id,city\n");
    for n in 1..=300 {
        let city = if n % 100 == 0 { "Lima" } else { "Oslo" };
        body.push_str(&format!("{n},{city}\n"));
    }
    body
}

fn capsule(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

fn selected_row(harness: &Harness) -> String {
    harness
        .text_of(".ds-row[aria-selected=true]")
        .unwrap_or_default()
}

#[test]
fn find_in_a_table_lists_the_cells_marks_them_and_walks_every_hit() {
    let (_dir, mut harness) = open("cities.csv", &cities());
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
    type_text(&mut harness, "lima");
    let listed = harness.text_of(".ds-palette").unwrap_or_default();
    assert!(
        listed.contains("In This File")
            && listed.contains("Row 100 \u{b7} city: Lima")
            && listed.contains("Row 300 \u{b7} city: Lima"),
        "the cells are listed by their row and their column: {listed}"
    );
    assert!(
        selected_row(&harness).contains("100"),
        "the cursor is on the first hit's row: {:?}",
        selected_row(&harness)
    );
    assert!(harness.count(".viewer-hit") >= 1, "its cell is marked");
    assert_eq!(
        harness.count(".viewer-hit-current"),
        1,
        "and it is the current one"
    );
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(harness.count(".ds-palette"), 0, "Enter closes the palette");
    assert!(
        capsule(&harness).contains("1 of 3"),
        "{}",
        capsule(&harness)
    );
    chord(&mut harness, 'g');
    assert!(
        capsule(&harness).contains("2 of 3"),
        "{}",
        capsule(&harness)
    );
    assert!(
        selected_row(&harness).contains("200"),
        "{:?}",
        selected_row(&harness)
    );
    harness.send(Input::chord(
        &[ShortcutKey::Super, ShortcutKey::Shift],
        ShortcutKey::Char('g'),
    ));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("1 of 3"),
        "{}",
        capsule(&harness)
    );
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(harness.count(".viewer-hit"), 0, "Esc puts the find away");
}

#[test]
fn a_find_in_a_table_that_matches_nothing_says_so() {
    let (_dir, mut harness) = open("cities.csv", &cities());
    chord(&mut harness, 'f');
    type_text(&mut harness, "zzz");
    assert!(
        harness
            .text_of(".ds-palette")
            .unwrap_or_default()
            .contains("No matches")
    );
    assert_eq!(harness.count(".viewer-hit"), 0);
}

#[test]
fn the_answer_to_a_find_a_newer_query_replaced_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cities.csv");
    std::fs::write(&path, cities()).unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let gate = Gate::holding(&[WorkKind::Search]);
    let wiring = Wiring {
        workers: Some(gate.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    chord(&mut harness, 'f');
    // "l", "li", "lim" and "lima" are each a text asked for, and only the last is still typed.
    type_text(&mut harness, "lima");
    let asked = gate.held(WorkKind::Search);
    assert!(asked >= 2, "each text typed went to a worker: {asked}");
    // Every answer but the last is for a text that is no longer typed.
    for _ in 1..asked {
        gate.release_one(WorkKind::Search);
        settle(&mut harness);
        assert_eq!(
            harness.count(".viewer-hit"),
            0,
            "an old answer marks nothing"
        );
    }
    gate.release_one(WorkKind::Search);
    settle(&mut harness);
    assert!(
        harness.count(".viewer-hit") >= 1,
        "the last answer is the find"
    );
}

/// The height of the first mounted row of the table.
fn row_height(harness: &Harness) -> f32 {
    harness
        .rect(".ds-vtable-item")
        .expect("a row")
        .size
        .height
        .0
}

#[test]
fn zoom_scales_the_rows_of_a_table_and_actual_size_returns() {
    let (_dir, actual) = open("cities.csv", &cities());
    let base = row_height(&actual);
    // name, keys pressed, the row height as a share of the actual one
    const CASES: &[(&str, &[char], f32)] = &[
        ("in", &['+'], 1.25),
        ("in twice", &['+', '+'], 1.5),
        ("out", &['-'], 0.75),
        (
            "out past the floor stops at half",
            &['-', '-', '-', '-', '-'],
            0.5,
        ),
        ("in and back to actual size", &['+', '+', '0'], 1.0),
    ];
    for (name, keys, share) in CASES {
        let (_dir, mut harness) = open("cities.csv", &cities());
        for key in *keys {
            press(&mut harness, ShortcutKey::Char(*key));
        }
        let got = row_height(&harness);
        assert!(
            (got - base * share).abs() < 1.0,
            "{name}: a row is {got} px, wanted {} of {base}",
            share
        );
    }
}
