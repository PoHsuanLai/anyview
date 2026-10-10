//! Two writers on one store, standing in for two processes: each write merges into what is on
//! disk, so one never erases the other's entries.

use crate::support::viewed_file;

use anyview_core::{FormatKind, LineIndex, Resume};
use anyview_store::{HistoryCap, HistoryRead, StoreWriter, Viewed, read_history};
use std::fs;

const FILES_EACH: u64 = 20;

fn writer(root: &std::path::Path) -> StoreWriter {
    StoreWriter::new(root, HistoryCap::new(10_000).unwrap())
}

fn at_line(line: u32) -> Resume {
    Resume::Text {
        line: LineIndex(line),
    }
}

#[test]
fn writers_on_different_files_keep_each_others_entries() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let files: Vec<Vec<_>> = ["a", "b"]
        .iter()
        .map(|who| {
            (0..FILES_EACH)
                .map(|k| viewed_file(scratch.path(), &format!("{who}{k}.txt"), who))
                .collect()
        })
        .collect();

    std::thread::scope(|scope| {
        for (who, mine) in (0_u64..).zip(&files) {
            let root = &root;
            scope.spawn(move || {
                let writer = writer(root); // its own instance, as in its own process
                for (k, (path, stamp)) in (0_u64..).zip(mine) {
                    let at = at_line(u32::try_from(k + 1).unwrap());
                    let seen = Viewed(who * 1000 + k);
                    // In the history first: another writer's prune keeps what the history names.
                    writer
                        .record_view(path, FormatKind::PlainText, seen, &at)
                        .unwrap();
                    writer.save_resume(path, *stamp, &at).unwrap();
                }
            });
        }
    });

    let history = read_history(&root);
    assert_eq!(history.entries().len() as u64, 2 * FILES_EACH);
    let reader = writer(&root);
    for (path, stamp) in files.iter().flatten() {
        assert!(
            history.entries().iter().any(|entry| &entry.path == path),
            "{path:?} missing from the history"
        );
        assert!(
            reader.load_resume(path, *stamp).unwrap().is_some(),
            "{path:?} lost its view memory"
        );
    }
}

#[test]
fn the_last_writer_of_one_file_wins_and_only_that_entry() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let (shared, shared_stamp) = viewed_file(scratch.path(), "shared.txt", "x");
    let (other, _) = viewed_file(scratch.path(), "other.txt", "y");
    let (first, second) = (writer(&root), writer(&root));

    first
        .record_view(&shared, FormatKind::PlainText, Viewed(1), &at_line(10))
        .unwrap();
    first
        .save_resume(&shared, shared_stamp, &at_line(10))
        .unwrap();
    second
        .record_view(&other, FormatKind::PlainText, Viewed(2), &at_line(5))
        .unwrap();
    second
        .record_view(&shared, FormatKind::PlainText, Viewed(3), &at_line(30))
        .unwrap();
    second
        .save_resume(&shared, shared_stamp, &at_line(30))
        .unwrap();

    let read = read_history(&root);
    let seen: Vec<_> = read.entries().iter().map(|e| (&e.path, e.viewed)).collect();
    assert_eq!(seen, [(&shared, Viewed(3)), (&other, Viewed(2))]);
    assert_eq!(
        first.load_resume(&shared, shared_stamp).unwrap(),
        Some(at_line(30))
    );
}

#[test]
fn temporary_files_left_by_a_crashed_write_do_not_hurt_the_store() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let (path, stamp) = viewed_file(scratch.path(), "book.txt", "x");
    let store = writer(&root);
    store
        .record_view(&path, FormatKind::PlainText, Viewed(1), &at_line(2))
        .unwrap();
    store.save_resume(&path, stamp, &at_line(2)).unwrap();

    // A writer died after starting its temporary files and before renaming them.
    fs::write(root.join("history.json.tmp"), b"{\"entries\":[{\"pa").unwrap();
    let resume_dir = fs::read_dir(root.join("resume")).unwrap();
    for entry in resume_dir.flatten() {
        let mut name = entry.file_name();
        name.push(".tmp");
        fs::write(root.join("resume").join(name), b"{ half").unwrap();
    }

    assert!(matches!(read_history(&root), HistoryRead::Loaded(_)));
    assert_eq!(store.load_resume(&path, stamp).unwrap(), Some(at_line(2)));
    store
        .record_view(&path, FormatKind::PlainText, Viewed(2), &at_line(9))
        .unwrap();
    store.save_resume(&path, stamp, &at_line(9)).unwrap();
    assert_eq!(store.load_resume(&path, stamp).unwrap(), Some(at_line(9)));
    assert_eq!(read_history(&root).entries().len(), 1);
}
