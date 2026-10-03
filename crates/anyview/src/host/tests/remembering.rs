//! Where the person is, written once for a run of gestures and once more as the program ends.

use super::support::{NOW, PNG, probed};
use crate::host::{Remembering, Store};
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
async fn a_run_of_places_is_one_write_of_the_last_and_not_before_the_wait_is_over() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let store = store_in(dir.path());
    let remembering =
        Remembering::new(Arc::clone(&store), tokio::runtime::Handle::current(), EVERY);

    for line in 1..=20 {
        remembering.note(file.source.clone(), text(line));
    }
    assert_eq!(
        store
            .resume(file.source.path(), file.source.stamp())
            .unwrap(),
        None,
        "nothing is written while the person is still moving"
    );

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
    let remembering = Remembering::new(
        Arc::clone(&store),
        tokio::runtime::Handle::current(),
        Duration::from_secs(3600),
    );

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
