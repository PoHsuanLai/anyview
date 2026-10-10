//! A pane over the viewer's store keeps the views and places itself, on the host's workers; a pane
//! without one hands every place to its host.

use crate::support::{Host, hosted_over, text_file};
use anyview_core::Resume;
use anyview_store::{HistoryCap, StoreWriter, read_history};
use ds::prelude::ShortcutKey;
use ds_harness::{Driver, Input, Viewport};
use std::path::PathBuf;

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
        .map(|entry| entry.path.as_path())
        .collect();
    assert_eq!(recent, [file.as_path()], "the view is in the history");

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
    let before = host.workers.submitted();
    for _ in 0..PAGES {
        host.harness.send(Input::key(ShortcutKey::PageDown));
        host.settle();
    }
    // Each place differs from the last, so without coalescing every page would be its own write.
    // The policy runs on real time while the harness's is virtual, so the bound only claims fewer
    // writes than pages: a slow machine may let an interval or two pass.
    let writes = host.workers.submitted() - before;
    assert!(
        writes < PAGES,
        "{PAGES} pages made {writes} writes: settled gestures inside one interval share a write"
    );
}

#[test]
fn a_pane_without_a_store_hands_the_place_to_its_host() {
    let files = tempfile::tempdir().unwrap();
    let mut host = hosted_over(vec![(long_text(&files), None)], in_a_corner(), None);
    scroll_to_the_end(&mut host);
    assert!(host.asked_to_remember());
}
