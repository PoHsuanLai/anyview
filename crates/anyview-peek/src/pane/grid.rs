//! The first rows of a table and the top level of a tree, both as quire's `Table`.

use anyview_text::{RowLabel, TablePeeked, TreePeeked, TreeRow};
use dioxus::prelude::*;
use ds::components::lists::table::model::{Sorting, TableColumn, TableRow};
use ds::components::lists::table::view::Table;
use ds::prelude::Px;

/// The most columns the pane draws; the facts say how many the table has.
const MAX_COLUMNS: usize = 6;

/// How wide each column starts.
const COLUMN: Px = Px(120.0);

/// The peeked rows of a delimited table, under its header (or "Column 1", "Column 2"… when the
/// first row did not look like one).
pub(super) fn table(peeked: &TablePeeked) -> Element {
    let width = usize::try_from(peeked.columns.0)
        .unwrap_or(MAX_COLUMNS)
        .min(MAX_COLUMNS);
    let titles: Vec<String> = (0..width)
        .map(|at| match &peeked.header {
            Some(header) => header.get(at).cloned().unwrap_or_default(),
            None => format!("Column {}", at + 1),
        })
        .collect();
    let rows = peeked.rows.iter().enumerate().map(|(at, row)| {
        let cells = (0..width)
            .map(|column| cell(row.get(column).map_or("", String::as_str)))
            .collect();
        TableRow::new(at, row.first().cloned().unwrap_or_default(), cells)
    });
    grid(titles, rows.collect())
}

/// The top level of a JSON tree: each entry's key and a short rendering of its value.
pub(super) fn tree(peeked: &TreePeeked) -> Element {
    let rows = peeked.top.iter().enumerate().map(|(at, row)| {
        let key = key_text(row);
        let cells = vec![cell(&key), cell(&row.preview)];
        TableRow::new(at, key, cells)
    });
    grid(vec!["Key".to_owned(), "Value".to_owned()], rows.collect())
}

fn key_text(row: &TreeRow) -> String {
    match &row.label {
        RowLabel::Root => "(root)".to_owned(),
        RowLabel::Key(key) => key.clone(),
        RowLabel::Index(index) => index.to_string(),
    }
}

fn cell(text: &str) -> Element {
    rsx! { "{text}" }
}

/// `rows` under `titles`, read-only: no header sorts, and a click selects nothing.
fn grid(titles: Vec<String>, rows: Vec<TableRow<usize>>) -> Element {
    let columns: Vec<TableColumn<usize>> = titles
        .into_iter()
        .enumerate()
        .map(|(at, title)| TableColumn::new(at, title, COLUMN).sorting(Sorting::Fixed))
        .collect();
    rsx! {
        div { class: "anyview-grid",
            Table::<usize, usize> {
                label: "Preview",
                columns,
                rows,
                on_sort: move |_| {},
                onselect: move |_| {},
            }
        }
    }
}
