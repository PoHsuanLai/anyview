//! The writer and reader together on scratch directories: history, view memory, pruning, damage.

use crate::support;

use anyview_core::{FilePath, FormatKind, LineIndex, PageIndex, Permille, Resume, Zoom};
use anyview_store::{
    HistoryCap, HistoryRead, ResumeLabel, StoreError, StoreWriter, Viewed, read_history,
};
use std::fs;
use support::{replace_file, stamp_of, viewed_file};

const AT_PAGE_143: Resume = Resume::Pdf {
    page: PageIndex(142),
    offset: Permille(0),
    zoom: Zoom::Fit,
};

fn cap(count: usize) -> HistoryCap {
    HistoryCap::new(count).unwrap()
}

fn resume_files(root: &std::path::Path) -> usize {
    fs::read_dir(root.join("resume")).map_or(0, Iterator::count)
}

#[test]
fn a_recorded_view_is_read_back_with_its_label() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store"); // does not exist yet
    let (path, _) = viewed_file(scratch.path(), "book.pdf", "pdf");
    let writer = StoreWriter::new(&root, cap(10));

    let before = read_history(&root);
    let recorded = writer
        .record_view(&path, FormatKind::Pdf, Viewed(1000), &AT_PAGE_143)
        .unwrap();
    let after = read_history(&root);

    assert_eq!(
        (before.entries().len(), recorded.replaced_damaged),
        (0, None)
    );
    let [entry] = after.entries() else {
        panic!("expected one entry: {after:?}");
    };
    assert_eq!(entry.path, path);
    assert_eq!(entry.kind, FormatKind::Pdf);
    assert_eq!(entry.viewed, Viewed(1000));
    assert_eq!(entry.label, ResumeLabel::Page { number: 143 });
}

#[test]
fn viewing_again_replaces_the_entry_and_the_cap_drops_the_oldest() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let writer = StoreWriter::new(&root, cap(2));
    let files: Vec<FilePath> = ["a", "b", "c"]
        .iter()
        .map(|n| viewed_file(scratch.path(), n, n).0)
        .collect();

    for (i, file) in files.iter().enumerate() {
        writer
            .record_view(
                file,
                FormatKind::PlainText,
                Viewed(i as u64),
                &Resume::Nothing,
            )
            .unwrap();
    }
    let names = |read: &HistoryRead| -> Vec<(FilePath, u64)> {
        read.entries()
            .iter()
            .map(|e| (e.path.clone(), e.viewed.0))
            .collect()
    };
    assert_eq!(
        names(&read_history(&root)),
        vec![(files[2].clone(), 2), (files[1].clone(), 1)]
    );

    writer
        .record_view(
            &files[1],
            FormatKind::PlainText,
            Viewed(9),
            &Resume::Text { line: LineIndex(6) },
        )
        .unwrap();
    let read = read_history(&root);
    assert_eq!(
        names(&read),
        vec![(files[1].clone(), 9), (files[2].clone(), 2)]
    );
    assert_eq!(read.entries()[0].label, ResumeLabel::Line { number: 7 });
}

#[test]
fn view_memory_comes_back_only_for_the_same_file() {
    let scratch = tempfile::tempdir().unwrap();
    let writer = StoreWriter::new(scratch.path().join("store"), cap(10));
    let (path, stamp) = viewed_file(scratch.path(), "notes.txt", "one\ntwo\n");
    let resume = Resume::Text {
        line: LineIndex(41),
    };

    assert_eq!(
        writer.load_resume(&path, stamp).unwrap(),
        None,
        "nothing saved yet"
    );
    writer.save_resume(&path, stamp, &resume).unwrap();
    assert_eq!(writer.load_resume(&path, stamp).unwrap(), Some(resume));

    replace_file(&path, "a different file at the same path");
    assert_eq!(
        writer.load_resume(&path, stamp_of(path.as_path())).unwrap(),
        None,
        "a replaced file restores nothing"
    );
}

#[test]
fn saving_nothing_forgets_what_was_saved() {
    let scratch = tempfile::tempdir().unwrap();
    let writer = StoreWriter::new(scratch.path().join("store"), cap(10));
    let (path, stamp) = viewed_file(scratch.path(), "notes.txt", "x");
    writer
        .save_resume(&path, stamp, &Resume::Text { line: LineIndex(3) })
        .unwrap();
    writer.save_resume(&path, stamp, &Resume::Nothing).unwrap();
    assert_eq!(writer.load_resume(&path, stamp).unwrap(), None);
    writer.save_resume(&path, stamp, &Resume::Nothing).unwrap(); // already gone is fine
}

#[test]
fn recording_a_view_prunes_memory_of_gone_replaced_and_forgotten_files() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let writer = StoreWriter::new(&root, cap(3));
    let resume = Resume::Text { line: LineIndex(5) };
    let names = ["kept", "gone", "replaced", "forgotten"];
    let files: Vec<_> = names
        .iter()
        .map(|n| viewed_file(scratch.path(), n, "body"))
        .collect();
    // "forgotten" is the oldest, so the cap of 3 pushes it out of the history below.
    for (i, (path, stamp)) in files.iter().enumerate().rev() {
        writer
            .record_view(path, FormatKind::PlainText, Viewed(10 - i as u64), &resume)
            .unwrap();
        writer.save_resume(path, *stamp, &resume).unwrap();
    }
    assert_eq!(
        resume_files(&root),
        3,
        "the fourth was pruned as forgotten already"
    );
    let (kept, kept_stamp) = &files[0];
    let (gone, _) = &files[1];
    let (replaced, replaced_stamp) = &files[2];

    fs::remove_file(gone.as_path()).unwrap();
    replace_file(replaced, "new content");
    writer
        .record_view(kept, FormatKind::PlainText, Viewed(99), &resume)
        .unwrap();

    assert_eq!(
        resume_files(&root),
        1,
        "only the unchanged, present, remembered file stays"
    );
    assert_eq!(writer.load_resume(kept, *kept_stamp).unwrap(), Some(resume));
    assert_eq!(writer.load_resume(replaced, *replaced_stamp).unwrap(), None);
}

#[test]
fn a_damaged_history_is_replaced_and_reported_not_a_panic() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("history.json"), "{ not json").unwrap();
    let (path, _) = viewed_file(scratch.path(), "a.txt", "a");
    let writer = StoreWriter::new(&root, cap(5));

    assert!(matches!(
        read_history(&root).fault(),
        Some(StoreError::Corrupt { .. })
    ));
    let recorded = writer
        .record_view(&path, FormatKind::PlainText, Viewed(1), &Resume::Nothing)
        .unwrap();

    assert!(matches!(
        recorded.replaced_damaged,
        Some(StoreError::Corrupt { .. })
    ));
    assert_eq!(read_history(&root).entries().len(), 1);
}

#[test]
fn a_damaged_view_memory_file_is_an_error_on_load_and_deleted_by_pruning() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let writer = StoreWriter::new(&root, cap(5));
    let (path, stamp) = viewed_file(scratch.path(), "a.txt", "a");
    let resume = Resume::Text { line: LineIndex(1) };
    writer.save_resume(&path, stamp, &resume).unwrap();
    let record = fs::read_dir(root.join("resume"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&record, "[1, 2").unwrap();

    assert!(matches!(
        writer.load_resume(&path, stamp),
        Err(StoreError::Corrupt { .. })
    ));
    writer
        .record_view(&path, FormatKind::PlainText, Viewed(1), &resume)
        .unwrap();
    assert!(!record.exists(), "the damaged record is deleted");
    assert_eq!(writer.load_resume(&path, stamp).unwrap(), None);
}

#[test]
fn a_path_that_is_not_utf8_is_refused_and_writes_nothing() {
    use std::os::unix::ffi::OsStrExt;
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let writer = StoreWriter::new(&root, cap(5));
    let mut bytes = scratch.path().as_os_str().as_bytes().to_vec();
    bytes.extend_from_slice(b"/caf\xe9.txt");
    let path = FilePath::new(std::ffi::OsStr::from_bytes(&bytes)).unwrap();

    let result = writer.record_view(&path, FormatKind::PlainText, Viewed(1), &Resume::Nothing);
    assert!(
        matches!(result, Err(StoreError::PathNotUtf8 { .. })),
        "{result:?}"
    );
    assert_eq!(read_history(&root), HistoryRead::Absent);
}
