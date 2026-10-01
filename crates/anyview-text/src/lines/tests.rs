use super::*;
use crate::bytes::{FileBytes, HeldBytes};
use anyview_core::{ByteLen, FilePath, FileStamp, ModTime, Source};
use std::cell::Cell;

fn window(text: &TextLines<impl ByteSource>, from: u32, to: u32) -> Vec<String> {
    text.lines(LineIndex(from)..LineIndex(to)).unwrap()
}

fn numbered(count: u32) -> String {
    (0..count).map(|i| format!("line {i}\n")).collect()
}

fn names(from: u32, to: u32) -> Vec<String> {
    (from..to).map(|i| format!("line {i}")).collect()
}

#[test]
fn text_is_split_at_line_feeds_and_a_final_one_ends_the_last_line() {
    // name, text, lines
    const CASES: &[(&str, &str, &[&str])] = &[
        ("empty", "", &[]),
        ("one line", "a", &["a"]),
        ("trailing line feed", "a\n", &["a"]),
        ("two lines", "a\nb", &["a", "b"]),
        ("blank line kept", "a\n\nb\n", &["a", "", "b"]),
        ("only a line feed", "\n", &[""]),
        ("crlf", "a\r\nb\r\n", &["a", "b"]),
        ("a lone carriage return stays", "a\rb\n", &["a\rb"]),
        ("two trailing line feeds", "a\n\n", &["a", ""]),
    ];
    for (name, text, want) in CASES {
        assert_eq!(split(text), *want, "{name}");
    }
}

#[test]
fn a_window_returns_exactly_the_requested_lines_at_every_anchor_boundary() {
    let text = TextLines::open(HeldBytes::new(numbered(300))).unwrap();
    assert_eq!(text.line_count(), LineCount(300));
    // name, from, to
    const CASES: &[(&str, u32, u32)] = &[
        ("first line", 0, 1),
        ("start", 0, 10),
        ("just before an anchor", 62, 66),
        ("on an anchor", 64, 70),
        ("one past an anchor", 65, 66),
        ("spanning two anchors", 60, 130),
        ("the last line", 299, 300),
        ("everything", 0, 300),
    ];
    for (name, from, to) in CASES {
        assert_eq!(window(&text, *from, *to), names(*from, *to), "{name}");
    }
}

#[test]
fn a_range_outside_the_file_is_cut_to_the_lines_that_exist() {
    let text = TextLines::open(HeldBytes::new(numbered(5))).unwrap();
    assert_eq!(window(&text, 3, 99), names(3, 5), "past the end");
    assert_eq!(
        window(&text, 7, 9),
        Vec::<String>::new(),
        "wholly past the end"
    );
    assert_eq!(window(&text, 2, 2), Vec::<String>::new(), "empty");
    assert_eq!(window(&text, 4, 1), Vec::<String>::new(), "reversed");
}

#[test]
fn line_counts_follow_final_line_breaks() {
    // name, bytes, count
    const CASES: &[(&str, &[u8], u32)] = &[
        ("empty", b"", 0),
        ("one unterminated line", b"abc", 1),
        ("one terminated line", b"abc\n", 1),
        ("two lines", b"a\nb\n", 2),
        ("blank lines", b"\n\n\n", 3),
        ("crlf", b"a\r\nb\r\n", 2),
        ("bom only", b"\xEF\xBB\xBF", 0),
    ];
    for (name, bytes, count) in CASES {
        let text = TextLines::open(HeldBytes::new(*bytes)).unwrap();
        assert_eq!(text.line_count(), LineCount(*count), "{name}");
    }
}

#[test]
fn each_encoding_windows_to_the_same_lines() {
    let source = numbered(200);
    let utf16 = |little: bool| -> Vec<u8> {
        let mut out = if little {
            vec![0xFF, 0xFE]
        } else {
            vec![0xFE, 0xFF]
        };
        for unit in source.encode_utf16() {
            out.extend_from_slice(&if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        out
    };
    let mut with_mark = vec![0xEF, 0xBB, 0xBF];
    with_mark.extend_from_slice(source.as_bytes());
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("utf-8", source.clone().into_bytes()),
        ("utf-8 with mark", with_mark),
        ("utf-16 le", utf16(true)),
        ("utf-16 be", utf16(false)),
    ];
    for (name, bytes) in cases {
        let text = TextLines::open(HeldBytes::new(bytes)).unwrap();
        assert_eq!(text.line_count(), LineCount(200), "{name} count");
        assert_eq!(window(&text, 60, 70), names(60, 70), "{name} window");
        assert_eq!(window(&text, 199, 200), names(199, 200), "{name} last");
    }
}

#[test]
fn a_legacy_encoded_file_decodes_with_the_fallback() {
    let text = TextLines::open(HeldBytes::new(b"caf\xE9\nna\xEFve\n".as_slice())).unwrap();
    assert_eq!(text.encoding(), TextCodec::Windows1252);
    assert_eq!(window(&text, 0, 2), ["café", "naïve"]);
}

/// Counts the bytes handed out, to show a window does not read the whole file.
struct Counting {
    inner: HeldBytes,
    read: Cell<u64>,
}

impl ByteSource for Counting {
    fn byte_len(&self) -> ByteLen {
        self.inner.byte_len()
    }

    fn read(&self, range: Range<u64>) -> Result<Vec<u8>, TextError> {
        let bytes = self.inner.read(range)?;
        self.read.set(self.read.get() + bytes.len() as u64);
        Ok(bytes)
    }
}

#[test]
fn a_window_deep_in_a_huge_file_reads_a_small_part_of_it() {
    let big = numbered(1_000_000);
    let file_len = big.len() as u64;
    let source = Counting {
        inner: HeldBytes::new(big),
        read: Cell::new(0),
    };
    let text = TextLines::open(source).unwrap();
    assert_eq!(text.line_count(), LineCount(1_000_000));
    assert!(
        text.bytes.read.get() >= file_len,
        "opening streams the file once"
    );
    text.bytes.read.set(0);
    assert_eq!(window(&text, 500_000, 500_040), names(500_000, 500_040));
    let spent = text.bytes.read.get();
    assert!(
        spent <= 128 * 1024,
        "{spent} bytes read of a {file_len} byte file for 40 lines"
    );
}

#[test]
fn a_file_on_disk_windows_the_same_as_the_same_bytes_in_memory() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("server.log");
    std::fs::write(&path, numbered(500)).unwrap();
    let stamp = FileStamp {
        len: ByteLen(std::fs::metadata(&path).unwrap().len()),
        modified: ModTime(0),
    };
    let src = Source::new(FilePath::new(&path).unwrap(), stamp);
    let on_disk = TextLines::open(FileBytes::open(&src).unwrap()).unwrap();
    assert_eq!(on_disk.line_count(), LineCount(500));
    assert_eq!(window(&on_disk, 250, 260), names(250, 260));
}

#[test]
fn a_missing_file_names_its_path() {
    let stamp = FileStamp {
        len: ByteLen(0),
        modified: ModTime(0),
    };
    let src = Source::new(FilePath::new("/nonexistent/x.log").unwrap(), stamp);
    assert!(matches!(FileBytes::open(&src), Err(TextError::Read { .. })));
}
