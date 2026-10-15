use super::*;
use anyview_core::FileName;
use anyview_core::work::Stop;

fn parse(text: &str, mode: HeaderMode) -> Table {
    Table::parse(text, Delimiter::Comma, mode).unwrap()
}

fn cells(table: &Table, from: u32, to: u32) -> Vec<Vec<String>> {
    table.rows(RowIndex(from)..RowIndex(to)).unwrap()
}

/// `rows` as the literals the tests compare with.
fn text<const N: usize>(rows: &[[&str; N]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
        .collect()
}

/// The bytes as a file the table index reads.
fn file(bytes: &[u8]) -> Input {
    Input::from((FileName::new("t.csv").unwrap(), bytes.to_vec()))
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
    assert_eq!(cells(&table, 0, 2), text(&[["Ann", "30"], ["Bob", "25"]]));
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
        text(&[["a", "b"], ["x, y", "say \"hi\""], ["two\nlines", "z"]])
    );
}

#[test]
fn tab_separated_tables_use_tabs_and_ignore_commas() {
    let table = Table::parse("a,b\tc\n1\t2,3\n", Delimiter::Tab, HeaderMode::Absent).unwrap();
    assert_eq!(cells(&table, 0, 2), text(&[["a,b", "c"], ["1", "2,3"]]));
    assert_eq!(table.separator(), Separator::Tab);
}

#[test]
fn ragged_rows_keep_their_cells_and_the_width_is_the_widest() {
    let table = parse("a,b,c\n1\n1,2,3,4\n", HeaderMode::Absent);
    assert_eq!(table.columns(), ColumnCount(4));
    assert_eq!(cells(&table, 1, 2), text(&[["1"]]));
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
        let window = table.rows(RowIndex(*from)..RowIndex(*to)).unwrap();
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

/// Rows of `id,note` where every seventh note holds a comma and a line break, so a record is
/// not a line.
fn long_csv(rows: u32) -> String {
    let mut text = String::from("id,note\n");
    for row in 0..rows {
        if row % 7 == 3 {
            text.push_str(&format!("{row},\"line one\nline, two\"\n"));
        } else {
            text.push_str(&format!("{row},note {row}\n"));
        }
    }
    text
}

fn utf16(text: &str, mark: [u8; 2], unit: fn(u16) -> [u8; 2]) -> Vec<u8> {
    let mut bytes = mark.to_vec();
    for each in text.encode_utf16() {
        bytes.extend_from_slice(&unit(each));
    }
    bytes
}

#[test]
fn an_indexed_file_reads_the_rows_a_parse_of_its_text_holds() {
    let long = long_csv(300);
    let le = utf16("a,b\n1,2\n\"x\ny\",4\n", [0xFF, 0xFE], u16::to_le_bytes);
    let be = utf16("a,b\n1,2\n", [0xFE, 0xFF], u16::to_be_bytes);
    // name, bytes
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("plain", b"a,b\n1,2\n3,4\n".to_vec()),
        ("crlf", b"a,b\r\n1,2\r\n3,4\r\n".to_vec()),
        ("lone cr", b"a,b\r1,2\r3,4".to_vec()),
        ("blank lines", b"a,b\n\n1,2\n\n\n3,4\n".to_vec()),
        ("no final break", b"a,b\n1,2".to_vec()),
        (
            "a quoted line break",
            b"name,note\nAnn,\"two\nlines\"\nBob,x\n".to_vec(),
        ),
        ("a quoted quote", b"a\n\"say \"\"hi\"\"\"\n".to_vec()),
        ("ragged rows", b"a,b,c\n1\n1,2,3,4\n".to_vec()),
        ("an empty first cell", b",a\n,b\n".to_vec()),
        ("a byte-order mark", b"\xEF\xBB\xBFa,b\n1,2\n".to_vec()),
        ("latin-1", b"caf\xE9,b\n1,2\n".to_vec()),
        ("semicolons", b"a;b\n1,5;2\n".to_vec()),
        ("many windows", long.into_bytes()),
        ("utf-16 le", le),
        ("utf-16 be", be),
        ("empty", Vec::new()),
    ];
    for (name, bytes) in &cases {
        let held = Table::parse_bytes(bytes, Delimiter::Comma, HeaderMode::Detect).unwrap();
        let read = Table::read(file(bytes), Delimiter::Comma, HeaderMode::Detect).unwrap();
        assert_eq!(read.header(), held.header(), "{name}: header");
        assert_eq!(read.columns(), held.columns(), "{name}: columns");
        assert_eq!(read.separator(), held.separator(), "{name}: separator");
        assert_eq!(read.row_count(), held.row_count(), "{name}: row count");
        let all = RowIndex(0)..RowIndex(u32::MAX);
        assert_eq!(
            read.rows(all.clone()).unwrap(),
            held.rows(all).unwrap(),
            "{name}: rows"
        );
        assert_eq!(read.column_widths(), held.column_widths(), "{name}: widths");
    }
}

#[test]
fn a_row_far_into_an_indexed_file_is_read_alone() {
    let table = Table::read(
        file(long_csv(300).as_bytes()),
        Delimiter::Comma,
        HeaderMode::Detect,
    )
    .unwrap();
    assert_eq!(table.row_count(), RowCount(300));
    assert_eq!(table.cell(RowIndex(250), 0), "250");
    assert_eq!(table.cell(RowIndex(255), 1), "line one\nline, two");
    assert_eq!(table.cell(RowIndex(300), 0), "", "past the end reads empty");
    let ids: Vec<String> = table.column(0).collect();
    assert_eq!(ids.len(), 300);
    assert_eq!(ids[299], "299");
    assert_eq!(
        table.column(5).count(),
        300,
        "a missing column is empty cells"
    );
}

#[test]
fn a_file_longer_than_the_cap_indexes_its_whole_records_and_says_so() {
    const TEXT: &[u8] = b"a,b\n1,2\n3,4\n5,6\n";
    // name, cap, rows, coverage
    const CASES: &[(&str, u64, u32, Coverage)] = &[
        ("all of it", 99, 3, Coverage::Whole),
        ("a cut after a record", 12, 2, Coverage::Prefix),
        ("a cut inside a record", 10, 1, Coverage::Prefix),
    ];
    for (name, cap, rows, coverage) in CASES {
        let (table, _) = Table::open(
            file(TEXT),
            Delimiter::Comma,
            HeaderMode::Detect,
            ByteLen(*cap),
            &Deadline::NEVER,
        )
        .unwrap();
        assert_eq!(table.row_count(), RowCount(*rows), "{name}: rows");
        assert_eq!(table.coverage(), *coverage, "{name}: coverage");
    }
}

#[test]
fn a_table_whose_read_is_interrupted_is_not_returned_half_indexed() {
    let stop = Stop::new();
    stop.request();
    let read = Table::read_until(
        file(b"a,b\n1,2\n"),
        Delimiter::Comma,
        HeaderMode::Detect,
        &stop,
    );
    assert!(matches!(read, Err(TextError::Interrupted)), "{read:?}");
}
