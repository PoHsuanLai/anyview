//! Windowed reading and highlighting through the public API, on files.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{ByteLen, FilePath, FileStamp, LineIndex, ModTime, Source, SyntaxName};
use anyview_text::{CodeLines, FileBytes, Highlighter, LineCount, TextLines, TokenClass};
use support::fixture;

fn open(name: &str) -> TextLines<FileBytes> {
    let (src, _) = fixture(name);
    TextLines::open(FileBytes::open(&src).unwrap()).unwrap()
}

#[test]
fn a_fixture_file_is_windowed_by_line() {
    let text = open("notes.txt");
    assert_eq!(text.line_count(), LineCount(60));
    let lines = text.lines(LineIndex(57)..LineIndex(99)).unwrap();
    assert_eq!(
        lines,
        [
            "note 58: remember to water the plants",
            "note 59: remember to water the plants",
            "note 60: remember to water the plants",
        ]
    );
}

#[test]
fn a_code_file_is_highlighted_by_window_from_disk() {
    let highlighter = Highlighter::new();
    let syntax = highlighter.syntax(&SyntaxName::new("rust").unwrap());
    let mut code = CodeLines::new(&highlighter, open("sample.rs"), syntax);
    let window = code
        .highlight(&highlighter, LineIndex(14)..LineIndex(15))
        .unwrap();
    assert_eq!(window.len(), 1);
    assert_eq!(window[0].number, LineIndex(14));
    let string = window[0]
        .spans
        .iter()
        .find(|s| s.class == TokenClass::String);
    assert!(
        string.is_some_and(|s| s.text.contains("hello")),
        "{:?}",
        window[0]
    );
}

#[test]
fn a_large_log_in_a_scratch_directory_is_counted_and_windowed() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("big.log");
    let log: String = (0..200_000)
        .map(|i| format!("2024-05-01 12:00:00 request {i} ok\n"))
        .collect();
    std::fs::write(&path, &log).unwrap();
    let stamp = FileStamp {
        len: ByteLen(log.len() as u64),
        modified: ModTime(0),
    };
    let src = Source::new(FilePath::new(&path).unwrap(), stamp);
    let text = TextLines::open(FileBytes::open(&src).unwrap()).unwrap();
    assert_eq!(text.line_count(), LineCount(200_000));
    let window = text.lines(LineIndex(123_456)..LineIndex(123_458)).unwrap();
    assert_eq!(
        window,
        [
            "2024-05-01 12:00:00 request 123456 ok",
            "2024-05-01 12:00:00 request 123457 ok",
        ]
    );
}
