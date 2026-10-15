//! The table and tree stages under the harness: a very large table mounts only the rows near the
//! window, a tree shows its first level, and the headers come from the file.

use crate::support;

use anyview_ui::WorkKind;
use dioxus::prelude::Modifiers;
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, PointerAction, PointerInput, Query};
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

#[test]
fn cells_are_drawn_by_what_their_column_holds_with_a_status_line_under_them() {
    let mut body = String::from("name,qty,done,site,region\n");
    for n in 0..24 {
        let region = ["Asia", "Europe", "Africa"][n % 3];
        let site = if n == 1 { "" } else { "nyc.gov" };
        let done = if n % 2 == 0 { "true" } else { "false" };
        body.push_str(&format!(
            "city {n},\"{}\",{done},{site},{region}\n",
            1000 + n
        ));
    }
    let (_dir, harness) = open("kinds.csv", &body);
    assert!(harness.count(".viewer-cell-number") > 0, "numbers");
    assert!(harness.count(".viewer-cell-yes-no") > 0, "yes and no");
    assert_eq!(
        harness.text_of(".viewer-cell-yes-no").as_deref(),
        Some("\u{2713}"),
        "a yes is a tick"
    );
    assert!(harness.count(".viewer-cell-link") > 0, "links");
    assert!(harness.count(".viewer-cell-tag") > 0, "tags");
    assert_eq!(
        harness.text_of(".viewer-cell-empty").as_deref(),
        Some("\u{2014}"),
        "an empty cell is a dash"
    );
    assert_eq!(
        harness.text_of(".viewer-table-status").as_deref(),
        Some("24 rows \u{b7} 5 columns")
    );
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

/// The first row's cells: the columns held in place, then the ones that scroll.
fn first_row(harness: &Harness) -> String {
    let held = harness
        .text_of(".ds-vtable-held .ds-table-cells")
        .unwrap_or_default();
    let rest = harness
        .text_of(".ds-vtable-columns .ds-table-cells")
        .unwrap_or_default();
    format!("{held}{rest}")
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

/// A sheet of twelve wide columns, far wider than the window, with "needle" in the last column of
/// row 5 and in the first column of row 7.
fn wide() -> String {
    let heads: Vec<String> = (0..12).map(|column| format!("column{column}")).collect();
    let mut body = heads.join(",");
    body.push('\n');
    for row in 1..=10 {
        let cells: Vec<String> = (0..12)
            .map(|column| match (row, column) {
                (5, 11) | (7, 0) => "needle in a wide cell".to_owned(),
                _ => format!("filler of some width {row}-{column}"),
            })
            .collect();
        body.push_str(&cells.join(","));
        body.push('\n');
    }
    body
}

/// Whether the current hit's cell is in view, sideways: in a column that is held in place, or
/// inside the columns that scroll and so clear of the held ones.
fn hit_in_room(harness: &Harness) -> bool {
    let Some(cell) = harness.rect(".viewer-hit-current") else {
        return false;
    };
    let inside = |room: ds::base::geometry::units::Rect| {
        cell.origin.x.0 >= room.origin.x.0 - 1.0
            && cell.origin.x.0 + cell.size.width.0 <= room.origin.x.0 + room.size.width.0 + 1.0
    };
    let Some(held) = harness.rect(".ds-vtable-held") else {
        return false;
    };
    if harness.count(".viewer-held .viewer-hit-current") > 0 {
        return inside(held);
    }
    // The columns that scroll, from the held ones' edge to the table's: the scrolling box's own
    // rect moves with its scroll in Blitz, so it is not the room.
    harness.rect("[data-body=\"table\"]").is_some_and(|table| {
        let start = held.origin.x.0 + held.size.width.0;
        let mut room = table;
        room.origin.x.0 = start;
        room.size.width.0 = table.origin.x.0 + table.size.width.0 - start;
        inside(room)
    })
}

#[test]
fn a_hit_in_a_column_off_to_the_side_scrolls_the_table_sideways_to_its_cell() {
    let (_dir, mut harness) = open("wide.csv", &wide());
    let (room, body) = (
        harness.rect(".ds-vtable-columns").expect("the room"),
        harness.rect(".ds-vtable-track").expect("the columns"),
    );
    assert!(
        body.size.width.0 > room.size.width.0 * 2.0,
        "the columns are wider than the room: {} in {}",
        body.size.width.0,
        room.size.width.0
    );
    chord(&mut harness, 'f');
    type_text(&mut harness, "needle");
    press(&mut harness, ShortcutKey::Enter);
    settle(&mut harness);
    assert!(
        hit_in_room(&harness),
        "the first hit is in the last column, and the table went to it"
    );
    chord(&mut harness, 'g');
    settle(&mut harness);
    assert!(
        hit_in_room(&harness),
        "the next hit is in the first column, which is held in place and so in view"
    );
    assert_eq!(
        harness.count(".viewer-held .viewer-hit-current"),
        1,
        "and it is in the held pane"
    );
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
    assert_eq!(
        gate.replaced(WorkKind::Search),
        asked - 1,
        "every search but the newest was told to end"
    );
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

#[test]
fn numbers_kept_as_numbers_sort_by_value_not_by_their_digits() {
    let (_dir, mut harness) = open("counts.csv", "n,name\n10,a\n9,b\n100,c\n");
    press_head(&mut harness);
    // The gutter number and the held first column, row after row, then the other cells.
    let held = harness.text_of(".ds-vtable-held").unwrap_or_default();
    let rest = harness.text_of(".ds-vtable-rows").unwrap_or_default();
    assert!(held.contains("192103100"), "9, 10, 100: {held}");
    assert!(rest.contains("bac"), "their names follow them: {rest}");
}

/// The width of the `at`th header cell, counting the gutter as the first. The gutter and the
/// first column are held in place, in a header of their own.
fn head_width(harness: &Harness, at: usize) -> f32 {
    let selector = match at {
        1 | 2 => format!(".ds-vtable-held-head .ds-table-head:nth-child({at})"),
        _ => format!(".ds-vtable-head-track .ds-table-head:nth-child({})", at - 2),
    };
    harness.rect(&selector).expect("a header cell").size.width.0
}

/// A table with a short id column, a long description column and a flag column to take the rest.
fn described() -> String {
    let mut body = String::from("id,description,flag\n");
    for n in 1..=30 {
        body.push_str(&format!(
            "{n},the quick brown fox jumps over the lazy dog {n},y\n"
        ));
    }
    body
}

#[test]
fn each_column_is_as_wide_as_its_content_asks_and_the_widths_follow_zoom() {
    let (_dir, mut harness) = open("described.csv", &described());
    let (id, text) = (head_width(&harness, 2), head_width(&harness, 3));
    assert!(
        id * 2.0 < text,
        "the id column ({id} px) is far narrower than the description ({text} px)"
    );
    press(&mut harness, ShortcutKey::Char('+'));
    press(&mut harness, ShortcutKey::Char('+'));
    let zoomed = head_width(&harness, 3);
    assert!(
        (zoomed - text * 1.5).abs() < 2.0,
        "zoomed to 150%, the description is {zoomed} px, wanted {} px",
        text * 1.5
    );
}

/// A sheet of twelve rows, `n+1,name n`.
fn twelve() -> String {
    let mut body = String::from("id,name\n");
    for n in 0..12 {
        body.push_str(&format!("{},name{n}\n", n + 1));
    }
    body
}

/// The row at place `row`, in the columns that scroll.
fn row_at(row: usize) -> String {
    format!(".ds-vtable-columns .ds-vtable-item[data-row=\"{row}\"]")
}

/// Click the middle of `selector` with `held` down.
fn click_holding(harness: &mut Harness, selector: &str, held: Modifiers) {
    let at = harness.centre(selector).expect("a row on screen");
    let click = PointerInput::new(
        at,
        PointerAction::Click(ds::base::press::PointerButton::Primary),
    );
    harness.send(Input::Pointer(click.with_mods(held)));
    settle(harness);
}

fn status(harness: &Harness) -> String {
    harness.text_of(".viewer-table-status").unwrap_or_default()
}

#[test]
fn rows_are_picked_as_in_finders_list_and_copy_puts_them_on_the_clipboard() {
    let (_dir, mut harness) = open("picks.csv", &twelve());
    assert_eq!(status(&harness), "12 rows \u{b7} 2 columns");
    click_holding(&mut harness, &row_at(2), Modifiers::empty());
    assert_eq!(
        status(&harness),
        "12 rows \u{b7} 2 columns",
        "one row picked says nothing more"
    );
    // The row is drawn picked in both panes.
    assert_eq!(harness.count(".ds-vtable-item[data-selected=yes]"), 2);
    click_holding(&mut harness, &row_at(5), Modifiers::SHIFT);
    assert_eq!(status(&harness), "4 rows selected");
    click_holding(&mut harness, &row_at(9), Modifiers::SUPER);
    assert_eq!(status(&harness), "5 rows selected");
    chord(&mut harness, 'c');
    assert_eq!(
        harness.clipboard_text().as_deref(),
        Some("3\tname2\n4\tname3\n5\tname4\n6\tname5\n10\tname9"),
        "the picked rows, tab-separated, with no header"
    );
    click_holding(&mut harness, &row_at(4), Modifiers::SUPER);
    assert_eq!(
        status(&harness),
        "4 rows selected",
        "a toggle takes a row out"
    );
    press(&mut harness, ShortcutKey::Escape);
    assert_eq!(
        status(&harness),
        "12 rows \u{b7} 2 columns",
        "Esc narrows to the cursor's row"
    );
    assert_eq!(harness.count(".ds-vtable-item[data-selected=yes]"), 2);
    chord(&mut harness, 'a');
    assert_eq!(status(&harness), "12 rows selected");
    click_holding(&mut harness, &row_at(3), Modifiers::empty());
    assert_eq!(
        harness.count(".ds-vtable-item[data-selected=yes]"),
        2,
        "a plain click narrows to one row"
    );
}

#[test]
fn the_same_rows_stay_picked_when_a_sort_moves_them() {
    let (_dir, mut harness, gate) = held_sorts();
    click_holding(&mut harness, &row_at(0), Modifiers::empty());
    let picked = |harness: &Harness| {
        (
            harness.text_of(".ds-vtable-columns .ds-row[aria-selected=true]"),
            harness.attr(".ds-vtable-item[data-selected=yes]", "data-row"),
        )
    };
    assert_eq!(
        picked(&harness),
        (Some("b".to_owned()), Some("0".to_owned()))
    );
    press_head(&mut harness);
    assert_eq!(
        picked(&harness),
        (Some("b".to_owned()), Some("0".to_owned())),
        "the rows wait in their order, and so does the pick"
    );
    gate.release(WorkKind::Sort);
    settle(&mut harness);
    assert_eq!(
        picked(&harness),
        (Some("b".to_owned()), Some("1".to_owned())),
        "the sorted order puts the same row of the file one place down"
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

/// A window on a new file of `name` holding `body`, and what the window asks of its host.
fn editing(
    name: &str,
    body: &str,
) -> (
    tempfile::TempDir,
    Harness,
    support::Requests,
    anyview_ui::Edge,
    PathBuf,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize({
        let path = dir.path().join(name);
        std::fs::write(&path, body).unwrap();
        path
    })
    .unwrap();
    let (mut harness, requests, edge) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    harness.send(Input::pointer_move(Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }));
    settle(&mut harness);
    (dir, harness, requests, edge, path)
}

/// The first cell of the first row, which is held in place beside the row number.
const FIRST_CELL: &str = ".viewer-held:not(.viewer-row-number)";

#[test]
fn a_cell_is_typed_into_where_it_is_and_command_s_writes_the_file_with_only_that_cell_changed() {
    let (_dir, mut harness, requests, edge, path) =
        editing("scores.csv", "name,qty\nann,1\nbob,2\n");
    // A double-click types into the cell under it.
    let at = harness.centre(FIRST_CELL).expect("the first cell");
    harness.send(Input::click(at));
    harness.send(Input::click(at));
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-inline-edit"),
        1,
        "a field in the cell"
    );
    assert_eq!(
        harness.count(".ds-titlebar-edited"),
        0,
        "nothing is changed yet"
    );
    type_text(&mut harness, "z");
    // Tab takes the text and goes to the cell on the right; the first cell is done with.
    press(&mut harness, ShortcutKey::Tab);
    assert_eq!(
        harness.count(".viewer-inline-edit"),
        0,
        "Tab takes the text"
    );
    assert_eq!(
        harness.count(".viewer-cell-focus"),
        1,
        "the cursor is on a cell"
    );
    assert_eq!(
        harness.count(".ds-titlebar-edited"),
        1,
        "the title bar's dot says there are unsaved changes"
    );
    chord(&mut harness, 's');
    settle(&mut harness);
    let written = String::from_utf8(saved_bytes(&requests).expect("a save was asked")).unwrap();
    assert_eq!(
        written, "name,qty\nannz,1\nbob,2\n",
        "a double-click types after the cell's text, and only that cell differs"
    );
    // The host wrote it and says so: the dot goes.
    edge.saved(
        anyview_core::FilePath::new(&path).unwrap(),
        anyview_ui::SaveEnd::Written,
    );
    settle(&mut harness);
    assert_eq!(harness.count(".ds-titlebar-edited"), 0, "saved");
}

#[test]
fn return_takes_a_cell_and_goes_down_and_a_row_is_added_and_taken_away_from_the_keyboard() {
    let (_dir, mut harness, requests, _edge, _path) =
        editing("scores.csv", "name,qty\nann,1\nbob,2\n");
    press(&mut harness, ShortcutKey::Down);
    press(&mut harness, ShortcutKey::Right);
    // A key typed on the picked cell begins typing into it; Return takes the text and goes down.
    type_text(&mut harness, "x");
    assert_eq!(harness.count(".viewer-inline-edit"), 1);
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(harness.count(".viewer-inline-edit"), 0);
    // Option Down adds a row below the one the cursor is on, which is picked.
    harness.send(Input::chord(&[ShortcutKey::Alt], ShortcutKey::Down));
    settle(&mut harness);
    assert_eq!(
        status(&harness),
        "3 rows \u{b7} 2 columns",
        "a row was added"
    );
    // Delete takes the picked row away.
    press(&mut harness, ShortcutKey::Delete);
    assert_eq!(
        status(&harness),
        "2 rows \u{b7} 2 columns",
        "and taken away"
    );
    chord(&mut harness, 's');
    settle(&mut harness);
    let written = String::from_utf8(saved_bytes(&requests).expect("a save was asked")).unwrap();
    assert_eq!(
        written, "name,qty\nx,1\nbob,2\n",
        "a key typed on a picked cell takes its place; the added row is gone again"
    );
}

#[test]
fn a_json_value_is_typed_into_on_return_and_the_file_keeps_its_own_layout_when_saved() {
    let original = "{\n  \"name\": \"anyview\",\n  \"n\": 3\n}\n";
    let (_dir, mut harness, requests, _edge, _path) = editing("tree.json", original);
    // The root, then its first entry.
    press(&mut harness, ShortcutKey::Down);
    press(&mut harness, ShortcutKey::Down);
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(
        harness.count(".viewer-inline-edit"),
        1,
        "the value is a field"
    );
    type_text(&mut harness, "!");
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(harness.count(".viewer-inline-edit"), 0);
    chord(&mut harness, 's');
    settle(&mut harness);
    let written = String::from_utf8(saved_bytes(&requests).expect("a save was asked")).unwrap();
    assert_eq!(
        written, "{\n  \"name\": \"anyview!\",\n  \"n\": 3\n}\n",
        "the value changed, and the indentation and the other entry are as they were"
    );
}

#[test]
fn command_z_in_a_cell_takes_back_the_typing_before_any_edit() {
    let (_dir, mut harness, _requests, _edge, _path) =
        editing("scores.csv", "name,qty\nann,1\nbob,2\n");
    let at = harness.centre(FIRST_CELL).expect("the first cell");
    harness.send(Input::click(at));
    harness.send(Input::click(at));
    settle(&mut harness);
    type_text(&mut harness, "z");
    // Numbers' first Undo while a cell is typed in takes back the typing; the field stays open.
    chord(&mut harness, 'z');
    assert_eq!(harness.count(".viewer-inline-edit"), 1, "still typing");
    press(&mut harness, ShortcutKey::Enter);
    assert_eq!(
        harness.count(".ds-titlebar-edited"),
        0,
        "the cell holds what it held, so nothing is changed"
    );
}
