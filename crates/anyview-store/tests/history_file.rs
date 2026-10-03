//! The history file through the public read API, in scratch directories.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use anyview_core::{FilePath, FormatKind};
use anyview_store::{
    History, HistoryEntry, HistoryRead, ResumeLabel, StoreError, StoreOp, Viewed, read_history,
};
use std::fs;

fn sample() -> History {
    History {
        entries: vec![HistoryEntry {
            path: FilePath::new("/books/a.pdf").unwrap(),
            kind: FormatKind::Pdf,
            viewed: Viewed(42),
            label: ResumeLabel::Page { number: 143 },
        }],
    }
}

#[test]
fn a_missing_root_reads_as_absent() {
    let scratch = tempfile::tempdir().unwrap();
    let read = read_history(&scratch.path().join("never-created"));
    assert_eq!(read, HistoryRead::Absent);
    assert_eq!(read.entries(), &[]);
    assert_eq!(read.fault(), None);
}

#[test]
fn an_existing_file_is_read_back_whole() {
    let scratch = tempfile::tempdir().unwrap();
    fs::write(
        scratch.path().join("history.json"),
        serde_json::to_vec(&sample()).unwrap(),
    )
    .unwrap();
    let read = read_history(scratch.path());
    assert_eq!(read, HistoryRead::Loaded(sample()));
    assert_eq!(read.entries(), sample().entries.as_slice());
}

#[test]
fn a_corrupt_file_is_unavailable_with_a_typed_reason() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("history.json");
    // A history cut off mid-entry, as a crash without rename would leave it.
    let full = serde_json::to_vec(&sample()).unwrap();
    fs::write(&path, &full[..full.len() / 2]).unwrap();
    let read = read_history(scratch.path());
    assert!(
        matches!(read.fault(), Some(StoreError::Corrupt { path: p, .. }) if *p == path),
        "{read:?}"
    );
    assert_eq!(read.entries(), &[]);
}

#[test]
fn a_file_with_a_relative_path_is_corrupt_not_a_panic() {
    let scratch = tempfile::tempdir().unwrap();
    fs::write(
        scratch.path().join("history.json"),
        r#"{"entries":[{"path":"a.pdf","kind":"pdf","viewed":1,"label":{"kind":"unlabelled"}}]}"#,
    )
    .unwrap();
    assert!(matches!(
        read_history(scratch.path()).fault(),
        Some(StoreError::Corrupt { .. })
    ));
}

#[test]
fn a_history_that_is_a_directory_is_an_io_fault() {
    let scratch = tempfile::tempdir().unwrap();
    fs::create_dir(scratch.path().join("history.json")).unwrap();
    assert!(matches!(
        read_history(scratch.path()).fault(),
        Some(StoreError::Io {
            op: StoreOp::Read,
            ..
        })
    ));
}
