//! The shared worker over the viewer's own look: a file asked for comes back as its card, a file
//! that cannot be read as an unavailable one, and a hostile one never stops the worker.

use crate::support;

use anyview_core::{FileName, FilePath, Input};
use anyview_fs::OnDisk;
use anyview_peek::{AnyPeeked, Body, PeekWorker, Unavailable, WorkerConfig};
use std::sync::mpsc;
use std::time::Duration;
use support::pane_looking;

fn worker() -> PeekWorker<AnyPeeked> {
    PeekWorker::looking(WorkerConfig::default(), pane_looking())
}

fn card(worker: &PeekWorker<AnyPeeked>, input: Input) -> AnyPeeked {
    let (sender, receiver) = mpsc::channel();
    worker.ask(input, move |card| {
        let _ = sender.send(card);
    });
    receiver.recv_timeout(Duration::from_secs(60)).unwrap()
}

#[test]
fn bytes_are_looked_at_and_a_missing_file_is_an_unavailable_card() {
    let worker = worker();
    let text = Input::from((
        FileName::new("note.txt").unwrap(),
        b"first\nsecond\n".to_vec(),
    ));
    let looked = card(&worker, text);
    assert_eq!(looked.name, "note.txt");
    assert!(matches!(looked.body, Body::Plain(_)), "{:?}", looked.body);

    let dir = tempfile::tempdir().unwrap();
    let gone = FilePath::new(dir.path().join("gone.txt")).unwrap();
    let missing = card(&worker, gone.on_disk());
    assert_eq!(missing.name, "gone.txt");
    assert!(
        matches!(missing.body, Body::Unavailable(Unavailable::Missing)),
        "{:?}",
        missing.body
    );
}

#[test]
fn a_hostile_file_is_one_unavailable_card_and_the_worker_goes_on() {
    let worker = worker();
    // Not a PNG, though it says it is.
    let hostile = Input::from((FileName::new("lie.png").unwrap(), vec![0x89; 64]));
    let first = card(&worker, hostile);
    assert!(
        matches!(first.body, Body::Unavailable(_) | Body::FactsOnly(_)),
        "{:?}",
        first.body
    );
    let next = Input::from((FileName::new("after.txt").unwrap(), b"fine\n".to_vec()));
    assert!(matches!(card(&worker, next).body, Body::Plain(_)));
}
