//! Spreadsheets that state a grid or a cell count far beyond what they hold: they open into a
//! bounded table (an allocation that fails aborts the process, so a test that returns has proved
//! the claim never became a grid).

use crate::support;

use anyview_core::Peek;
use anyview_fs::OnDisk;
use anyview_text::{Coverage, RowIndex, SHEET_ROWS, TablePeek, WORKBOOK_CELLS, Workbook};
use support::{budget, workbook_of_xml};

/// A sheet whose only cells are `cells`, given as references.
fn sheet_of(cells: &[&str]) -> String {
    let rows: String = cells
        .iter()
        .map(|at| {
            let row: String = at.chars().filter(char::is_ascii_digit).collect();
            format!(r#"<row r="{row}"><c r="{at}" t="inlineStr"><is><t>x</t></is></c></row>"#)
        })
        .collect();
    format!(r#"<dimension ref="A1:XFD1048576"/>{rows}"#)
}

#[test]
fn two_cells_a_million_rows_and_sixteen_thousand_columns_apart_do_not_become_a_grid() {
    let dir = tempfile::tempdir().unwrap();
    // name, cells
    let cases: [(&str, &[&str]); 2] = [
        ("opposite corners", &["A1", "XFD1048576"]),
        ("a tall thin pair", &["A1", "XFD2000"]),
    ];
    for (name, cells) in cases {
        let (src, sniffed) =
            workbook_of_xml(dir.path(), "sparse.xlsx", &["Sheet"], &[sheet_of(cells)]);
        let book = Workbook::open(src.on_disk()).unwrap();
        let table = &book.sheets()[0].table;
        let held = table.row_count().0 as usize * table.columns().0 as usize;
        assert!(held <= WORKBOOK_CELLS, "{name}: a grid of {held} cells");
        assert_eq!(table.cell(RowIndex(0), 0), "x", "{name}");
        let peeked = TablePeek::peek(&src.on_disk(), &sniffed, &budget(1_000_000)).unwrap();
        assert!(peeked.rows.len() <= 40, "{name}");
    }
}

#[test]
fn a_sheet_of_millions_of_cells_is_read_only_as_far_as_the_caps_reach() {
    let dir = tempfile::tempdir().unwrap();
    // 60 000 rows of 20 empty cells: 1.2 million cells, past the peek's million.
    let rows: String = (1..=60_000)
        .map(|r| {
            let cells: String = (0..20)
                .map(|c| format!(r#"<c r="{}{r}"/>"#, (b'A' + c) as char))
                .collect();
            format!(r#"<row r="{r}">{cells}</row>"#)
        })
        .collect();
    let (src, sniffed) = workbook_of_xml(dir.path(), "bomb.xlsx", &["Sheet"], &[rows]);
    let peeked = TablePeek::peek(&src.on_disk(), &sniffed, &budget(64 * 1024 * 1024)).unwrap();
    assert!(peeked.rows.len() <= 40);
    let book = Workbook::open_start(src.on_disk()).unwrap();
    assert_eq!(book.sheets().len(), 1);
    assert!(book.sheets()[0].table.row_count().0 as usize <= SHEET_ROWS);
}

#[test]
fn a_sheet_with_more_rows_than_the_cap_keeps_the_start_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let rows: String = (1..=SHEET_ROWS + 500)
        .map(|r| format!(r#"<row r="{r}"><c r="A{r}"><v>{r}</v></c></row>"#))
        .collect();
    let (src, _) = workbook_of_xml(dir.path(), "tall.xlsx", &["Sheet"], &[rows]);
    let book = Workbook::open(src.on_disk()).unwrap();
    let table = &book.sheets()[0].table;
    assert_eq!(table.coverage(), Coverage::Prefix);
    // The header guess may take the first row, and the cap counts it.
    assert!(table.row_count().0 as usize >= SHEET_ROWS - 1);
    assert!(table.row_count().0 as usize <= SHEET_ROWS);
}
