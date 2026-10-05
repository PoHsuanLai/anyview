use super::*;

fn parse(text: &str, mode: HeaderMode) -> Table {
    Table::parse(text, Delimiter::Comma, mode).unwrap()
}

fn cells(table: &Table, from: u32, to: u32) -> Vec<Vec<&str>> {
    table
        .rows(RowIndex(from)..RowIndex(to))
        .iter()
        .map(|row| row.iter().map(String::as_str).collect())
        .collect()
}

#[test]
fn a_detected_header_is_set_apart_from_the_rows() {
    let table = parse("name,age\nAnn,30\nBob,25\n", HeaderMode::Detect);
    assert_eq!(
        table.header(),
        Some(&["name".to_owned(), "age".to_owned()][..])
    );
    assert_eq!(table.row_count(), RowCount(2));
    assert_eq!(table.columns(), ColumnCount(2));
    assert_eq!(cells(&table, 0, 2), [["Ann", "30"], ["Bob", "25"]]);
}

#[test]
fn the_header_mode_overrides_the_guess() {
    const TEXT: &str = "Ann,30\nBob,25\n";
    let detected = parse(TEXT, HeaderMode::Detect);
    assert_eq!(detected.header(), None, "numbers in the first row: data");
    assert_eq!(detected.row_count(), RowCount(2));
    let forced = parse(TEXT, HeaderMode::Present);
    assert_eq!(forced.header().map(<[String]>::len), Some(2));
    assert_eq!(forced.row_count(), RowCount(1));
    let none = parse("name,age\nAnn,30\n", HeaderMode::Absent);
    assert_eq!(none.header(), None);
    assert_eq!(none.row_count(), RowCount(2));
}

#[test]
fn quoted_cells_keep_commas_quotes_and_line_breaks() {
    let table = parse(
        "a,b\n\"x, y\",\"say \"\"hi\"\"\"\n\"two\nlines\",z\n",
        HeaderMode::Absent,
    );
    assert_eq!(
        cells(&table, 0, 3),
        [
            vec!["a", "b"],
            vec!["x, y", "say \"hi\""],
            vec!["two\nlines", "z"]
        ]
    );
}

#[test]
fn tab_separated_tables_use_tabs_and_ignore_commas() {
    let table = Table::parse("a,b\tc\n1\t2,3\n", Delimiter::Tab, HeaderMode::Absent).unwrap();
    assert_eq!(cells(&table, 0, 2), [["a,b", "c"], ["1", "2,3"]]);
    assert_eq!(table.separator(), Separator::Tab);
}

#[test]
fn ragged_rows_keep_their_cells_and_the_width_is_the_widest() {
    let table = parse("a,b,c\n1\n1,2,3,4\n", HeaderMode::Absent);
    assert_eq!(table.columns(), ColumnCount(4));
    assert_eq!(cells(&table, 1, 2), [["1"]]);
    assert_eq!(table.cell(RowIndex(1), 0), "1");
    assert_eq!(table.cell(RowIndex(1), 3), "", "a short row reads empty");
    assert_eq!(table.cell(RowIndex(9), 0), "", "a missing row reads empty");
}

#[test]
fn row_windows_are_cut_to_the_rows_that_exist() {
    let text: String = (0..10).map(|i| format!("r{i},x\n")).collect();
    let table = parse(&text, HeaderMode::Absent);
    // name, from, to, first row, count
    const CASES: &[(&str, u32, u32, Option<&str>, usize)] = &[
        ("start", 0, 3, Some("r0"), 3),
        ("middle", 4, 6, Some("r4"), 2),
        ("past the end", 8, 99, Some("r8"), 2),
        ("wholly past the end", 20, 30, None, 0),
        ("empty", 5, 5, None, 0),
        ("reversed", 6, 2, None, 0),
    ];
    for (name, from, to, first, count) in CASES {
        let window = table.rows(RowIndex(*from)..RowIndex(*to));
        assert_eq!(window.len(), *count, "{name}");
        assert_eq!(window.first().map(|r| r[0].as_str()), *first, "{name}");
    }
}

#[test]
fn an_empty_file_is_an_empty_table() {
    let table = parse("", HeaderMode::Detect);
    assert_eq!(table.row_count(), RowCount(0));
    assert_eq!(table.columns(), ColumnCount(0));
    assert_eq!(table.header(), None);
    assert!(
        Table::parse("", Delimiter::Comma, HeaderMode::Present)
            .unwrap()
            .header()
            .is_none()
    );
}

#[test]
fn bytes_are_decoded_before_they_are_parsed() {
    // name, bytes, first cell
    const CASES: &[(&str, &[u8], &str)] = &[
        ("utf-8 with a mark", b"\xEF\xBB\xBFa,b\n", "a"),
        ("windows-1252", b"caf\xE9,b\n", "café"),
        ("utf-16 le", b"\xFF\xFEa\0,\0b\0\n\0", "a"),
    ];
    for (name, bytes, first) in CASES {
        let table = Table::parse_bytes(bytes, Delimiter::Comma, HeaderMode::Absent).unwrap();
        assert_eq!(table.cell(RowIndex(0), 0), *first, "{name}");
    }
}
