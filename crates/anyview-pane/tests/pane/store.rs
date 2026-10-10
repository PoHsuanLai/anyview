//! A pane over the viewer's store keeps the views and places itself, on the host's workers; a pane
//! without one hands every place to its host.

use crate::support::{Host, VIEWED_AT, hosted_over, text_file};
use anyview_core::Resume;
use anyview_store::{HistoryCap, StoreWriter, read_history};
use anyview_ui::REMEMBER_EVERY;
use ds::prelude::ShortcutKey;
use ds_harness::{Driver, Input, Viewport};
use std::path::PathBuf;
use std::time::Duration;

fn in_a_corner() -> Viewport {
    Viewport {
        width: 480,
        height: 320,
        scale_percent: 100,
    }
}

fn long_text(dir: &tempfile::TempDir) -> PathBuf {
    text_file(dir.path(), "long.txt", "line", 400)
}

fn scroll_to_the_end(host: &mut Host) {
    host.harness.send(Input::key(ShortcutKey::End));
    host.settle();
}

#[test]
fn a_store_backed_pane_records_the_view_and_keeps_the_place_without_asking_the_host() {
    let files = tempfile::tempdir().unwrap();
    let file = long_text(&files);
    let root = tempfile::tempdir().unwrap();
    let mut host = hosted_over(vec![(file.clone(), None)], in_a_corner(), Some(root.path()));
    scroll_to_the_end(&mut host);

    let history = read_history(root.path());
    let recent: Vec<_> = history
        .entries()
        .iter()
        .map(|entry| (entry.path.as_path(), entry.viewed))
        .collect();
    assert_eq!(
        recent,
        [(file.as_path(), VIEWED_AT)],
        "the view is in the history, stamped with the edge's clock"
    );

    let source = host.source();
    let kept = StoreWriter::new(root.path(), HistoryCap::DEFAULT)
        .load_resume(source.path(), source.stamp())
        .unwrap();
    assert!(
        kept.is_some_and(|place| place != Resume::Nothing),
        "the place was written"
    );
    assert!(!host.asked_to_remember(), "and the host was not asked");
    assert_eq!(
        host.showing().as_deref(),
        Some("long.txt"),
        "though it hears the file opened"
    );
}

#[test]
fn many_settled_gestures_make_few_writes() {
    const PAGES: usize = 12;
    let files = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut host = hosted_over(
        vec![(long_text(&files), None)],
        in_a_corner(),
        Some(root.path()),
    );
    let before = host.workers.kept();
    for _ in 0..PAGES {
        host.harness.send(Input::key(ShortcutKey::PageDown));
        host.settle();
    }
    // Each place differs from the last, so without coalescing every page would be its own write.
    // Each settle lets 300 ms of the harness's clock pass, so the writes are one in every interval
    // (or two settles) at most, fewer than the pages.
    let writes = host.workers.kept() - before;
    assert!(
        writes < PAGES,
        "{PAGES} pages made {writes} writes: settled gestures inside one interval share a write"
    );
}

#[test]
fn a_place_held_back_is_written_when_its_interval_ends() {
    let files = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut host = hosted_over(
        vec![(long_text(&files), None)],
        in_a_corner(),
        Some(root.path()),
    );
    // The first place is written at once; the pages after it, a moment apart, are inside its
    // interval and held back.
    host.harness.send(Input::key(ShortcutKey::PageDown));
    host.settle();
    for _ in 0..2 {
        host.harness.send(Input::key(ShortcutKey::PageDown));
        host.harness.advance(Duration::from_millis(20));
    }
    let before = host.workers.kept();

    // No further input: the interval ends by itself and the held place is written.
    host.harness.advance(REMEMBER_EVERY * 2);
    assert!(
        host.workers.kept() > before,
        "the held place is written when its interval ends"
    );
}

#[test]
fn a_pane_without_a_store_hands_the_place_to_its_host() {
    let files = tempfile::tempdir().unwrap();
    let mut host = hosted_over(vec![(long_text(&files), None)], in_a_corner(), None);
    scroll_to_the_end(&mut host);
    assert!(host.asked_to_remember());
}
