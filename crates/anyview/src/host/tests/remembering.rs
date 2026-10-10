//! The wiring of where the person is to the store: the policy's table is anyview-machines'. A run
//! of gestures ends with the last place kept, and a place waiting as the program ends is written.

use super::support::{NOW, PNG, probed};
use crate::host::{PlaceWriter, Store};
use anyview_core::{LineIndex, Resume};
use std::sync::Arc;
use std::time::Duration;

const EVERY: Duration = Duration::from_millis(80);

fn text(line: u32) -> Resume {
    Resume::Text {
        line: LineIndex(line),
    }
}

fn store_in(dir: &std::path::Path) -> Arc<Store> {
    Arc::new(Store::new(&dir.join("store"), Arc::new(|| NOW)))
}

#[tokio::test]
async fn a_run_of_places_ends_with_the_last_kept() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let store = store_in(dir.path());
    let remembering =
        PlaceWriter::new(Arc::clone(&store), tokio::runtime::Handle::current(), EVERY);

    for line in 1..=20 {
        remembering.note(file.source.clone(), text(line));
    }
    tokio::time::sleep(EVERY * 5).await;
    assert_eq!(
        store
            .resume(file.source.path(), file.source.stamp())
            .unwrap(),
        Some(text(20)),
        "the last place is the one kept"
    );
}

#[tokio::test]
async fn a_place_waiting_as_the_program_ends_is_written_by_the_flush() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let store = store_in(dir.path());
    let remembering = PlaceWriter::new(
        Arc::clone(&store),
        tokio::runtime::Handle::current(),
        Duration::from_secs(3600),
    );

    remembering.note(file.source.clone(), text(1));
    // The first place is written at once, on the pool; wait for it so it cannot land after the flush.
    for _ in 0..200 {
        let kept = store
            .resume(file.source.path(), file.source.stamp())
            .unwrap();
        if kept.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    remembering.note(file.source.clone(), text(7));
    let flushing = remembering.clone();
    tokio::task::spawn_blocking(move || flushing.flush())
        .await
        .unwrap();

    assert_eq!(
        store
            .resume(file.source.path(), file.source.stamp())
            .unwrap(),
        Some(text(7))
    );
}
