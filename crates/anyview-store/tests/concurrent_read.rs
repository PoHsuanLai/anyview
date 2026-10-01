//! A reader running while the writer replaces the history sees a whole history every time.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FilePath, FormatKind, Resume};
use anyview_store::{HistoryCap, HistoryRead, StoreWriter, Viewed, read_history};
use std::sync::atomic::{AtomicBool, Ordering};

const VIEWS: u64 = 120;

#[test]
fn a_concurrent_reader_sees_a_complete_history_at_every_moment() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().join("store");
    let writer = StoreWriter::new(&root, HistoryCap::new(10_000).unwrap());
    let done = AtomicBool::new(false);

    let reads = std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mut loaded = 0_u64;
            loop {
                let finished = done.load(Ordering::SeqCst);
                match read_history(&root) {
                    HistoryRead::Absent => {}
                    HistoryRead::Loaded(history) => {
                        // Entry k was viewed at time k and every view adds one file, so a whole
                        // history of n entries reads n, n-1, ..., 1. A half-written or mixed
                        // file would not.
                        let times: Vec<u64> = history.entries.iter().map(|e| e.viewed.0).collect();
                        let n = times.len() as u64;
                        let expected: Vec<u64> = (1..=n).rev().collect();
                        assert_eq!(times, expected, "a torn history was read");
                        loaded += 1;
                    }
                    HistoryRead::Unavailable(error) => panic!("reader saw damage: {error}"),
                }
                if finished {
                    return loaded;
                }
            }
        });
        for k in 1..=VIEWS {
            let path = FilePath::new(format!("/files/{k:04}/{}.txt", "x".repeat(200))).unwrap();
            writer
                .record_view(&path, FormatKind::PlainText, Viewed(k), &Resume::Nothing)
                .unwrap();
        }
        done.store(true, Ordering::SeqCst);
        reader.join().unwrap()
    });

    assert!(reads > 0, "the reader never saw the history");
    assert_eq!(read_history(&root).entries().len() as u64, VIEWS);
}
