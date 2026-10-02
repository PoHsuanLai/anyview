//! A viewer window under the harness: workers that run each job where it is submitted, a scratch
//! folder of real files for the arrow keys to walk, and the requests the window made of its host.
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};
use anyview_ui::{Edge, HostRequest, Launch, ViewerApp, Work, Workers};
use ds::prelude::Appearance;
use ds_harness::{Backend, Clock, Driver, Harness, HarnessConfig, Viewport};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The window's size in the tests.
pub const VIEW: Viewport = Viewport {
    width: 900,
    height: 600,
    scale_percent: 100,
};

/// Runs each job on the thread that submitted it, so a result is in the mailbox by the time the
/// harness looks.
#[derive(Debug)]
struct Inline;

impl Workers for Inline {
    fn submit(&self, work: Work) {
        work.run();
    }
}

/// The requests the window made of its host.
pub type Requests = Arc<Mutex<Vec<HostRequest>>>;

/// The fixture folders of the back-end crates.
pub fn fixture(crate_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_dir)
        .join("tests/fixtures")
        .join(name)
}

/// A scratch folder holding copies of `files` (crate folder, fixture name, name to give it), and
/// the paths in the order given.
pub fn folder(files: &[(&str, &str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let paths = files
        .iter()
        .map(|(crate_dir, name, called)| {
            let to = dir.path().join(called);
            std::fs::copy(fixture(crate_dir, name), &to).unwrap();
            std::fs::canonicalize(to).unwrap()
        })
        .collect();
    (dir, paths)
}

/// A viewer window opened on `paths[at]` with the whole list to walk.
pub fn window(paths: &[PathBuf], at: usize, appearance: Appearance) -> (Harness, Requests) {
    let entries: Vec<FilePath> = paths
        .iter()
        .map(|path| FilePath::new(path).unwrap())
        .collect();
    let current = entries[at].clone();
    let sequence = Sequence::starting_at(
        NonEmpty::from_vec(entries).unwrap(),
        &current,
        SequenceOrigin::Selection,
    )
    .unwrap();
    let requests: Requests = Arc::default();
    let seen = Arc::clone(&requests);
    let edge = Edge::new(Arc::new(Inline), move |request| {
        seen.lock().unwrap().push(request);
    });
    let launch = Launch {
        file: current,
        sequence: Some(sequence),
        appearance,
    };
    let config = HarnessConfig::new(VIEW)
        .with_clock(Clock::Virtual)
        .with_backend(Backend::Hybrid)
        .with_context(edge)
        .with_context(launch);
    let mut harness = Harness::new(ViewerApp, config);
    harness.advance(Duration::from_millis(500));
    (harness, requests)
}

/// Where a picture is saved for a person to look at.
pub fn shot(name: &str) -> Option<PathBuf> {
    std::env::var_os("ANYVIEW_SHOTS").map(|dir| Path::new(&dir).join(name))
}
