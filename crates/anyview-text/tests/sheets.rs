//! Tables read from disk the way the viewer opens them: a cap on rows, an encoding, a separator
//! the rows choose, and the sheets of a workbook built in the test.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{Delimiter, FormatDetail, FormatKind, Input, Peek};
use anyview_text::{
    CharWidth, Coverage, HeaderMode, Separator, TABLE_ROWS, Table, TablePeek, TableSource, Tally,
    Workbook,
};
use support::{budget, rows, workbook, written};

#[test]
fn a_file_with_more_rows_than_the_cap_keeps_the_start_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut text = String::from("id,name\n");
    for n in 0..TABLE_ROWS + 50 {
        text.push_str(&format!("{n},row{n}\n"));
    }
    let (src, _) = written(dir.path(), "big.csv", text.as_bytes());
    let table = Table::read(&src, Delimiter::Comma, HeaderMode::Detect).unwrap();
    // The cap counts the file's records, the header line among them.
    assert_eq!(table.row_count().0 as usize, TABLE_ROWS - 1);
    assert_eq!(table.coverage(), Coverage::Prefix);
    assert_eq!(table.header().map(<[String]>::len), Some(2));
    let small = written(dir.path(), "small.csv", b"a,b\n1,2\n").0;
    let whole = Table::read(&small, Delimiter::Comma, HeaderMode::Detect).unwrap();
    assert_eq!(whole.coverage(), Coverage::Whole);
}

#[test]
fn quotes_with_newlines_and_a_latin1_encoding_read_as_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = b"name,note\n".to_vec();
    bytes.extend_from_slice(b"Ren\xE9,\"two\nlines, and a \"\"quote\"\"\"\n");
    let (src, _) = written(dir.path(), "q.csv", &bytes);
    let table = Table::read(&src, Delimiter::Comma, HeaderMode::Detect).unwrap();
    assert_eq!(table.row_count().0, 1);
    assert_eq!(
        table.cell(anyview_text::RowIndex(0), 0),
        "René",
        "latin-1 decoded"
    );
    assert_eq!(
        table.cell(anyview_text::RowIndex(0), 1),
        "two\nlines, and a \"quote\""
    );
}

#[test]
fn a_semicolon_file_named_csv_is_split_by_its_semicolons() {
    let dir = tempfile::tempdir().unwrap();
    let (src, _) = written(dir.path(), "eu.csv", b"a;b\n1,5;2\n3,5;4\n");
    let table = Table::read(&src, Delimiter::Comma, HeaderMode::Detect).unwrap();
    assert_eq!(table.separator(), Separator::Semicolon);
    assert_eq!(table.columns().0, 2);
    assert_eq!(table.cell(anyview_text::RowIndex(0), 0), "1,5");
}

#[test]
fn column_widths_follow_the_longest_sampled_cell_within_bounds() {
    let table = Table::parse(
        "id,name,note\n1,Ann,a very long note that goes on and on and on and on past forty chars\n22,Bobby Tables,x\n",
        Delimiter::Comma,
        HeaderMode::Detect,
    )
    .unwrap();
    assert_eq!(
        table.column_widths(),
        vec![CharWidth(4), CharWidth(12), CharWidth(40)]
    );
}

#[test]
fn a_workbook_lists_its_sheets_in_order_with_numbers_and_text() {
    let dir = tempfile::tempdir().unwrap();
    let (src, sniffed) = workbook(
        dir.path(),
        "book.xlsx",
        &[
            (
                "People",
                &[&["name", "age"], &["Ann", "30"], &["Bob", "25.5"]],
            ),
            ("Empty", &[]),
            ("Notes", &[&["x"]]),
        ],
    );
    assert_eq!(sniffed.kind(), FormatKind::Table);
    assert!(matches!(sniffed.detail(), FormatDetail::Office(_)));
    let book = Workbook::open(&src).unwrap();
    let names: Vec<&str> = book.sheets().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["People", "Empty", "Notes"]);
    let people = &book.sheets()[0].table;
    assert_eq!(people.header().unwrap(), ["name", "age"]);
    assert_eq!(people.cell(anyview_text::RowIndex(0), 1), "30");
    assert_eq!(people.cell(anyview_text::RowIndex(1), 1), "25.5");
    assert_eq!(book.sheets()[1].table.row_count().0, 0);
}

#[test]
fn a_workbook_peeks_its_first_sheet_and_counts_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (src, sniffed) = workbook(
        dir.path(),
        "book.xlsx",
        &[
            (
                "People",
                &[&["name", "age"], &["Ann", "30"], &["Bob", "25"]],
            ),
            ("More", &[&["x"]]),
        ],
    );
    let peeked = TablePeek::peek(&Input::from(&src), &sniffed, &budget(1_000_000)).unwrap();
    assert_eq!(peeked.rows.len(), 2);
    assert_eq!(peeked.total_rows, Tally::Exact(2));
    assert!(matches!(
        peeked.source,
        TableSource::Sheet { sheets: 2, .. }
    ));
    assert_eq!(
        rows(&TablePeek::facts(&peeked)),
        support::expected(&[
            ("kind", "XLSX spreadsheet"),
            ("rows", "2"),
            ("columns", "2"),
            ("sheets", "2, showing People"),
        ])
    );
    assert!(
        TablePeek::peek(&Input::from(&src), &sniffed, &budget(100)).is_err(),
        "over the budget"
    );
}
