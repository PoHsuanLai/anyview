//! Files that are not what their name says: a FIFO, a device behind a link, a file over the
//! budget. None may hang a probe, be read past the budget, or reach a decoder.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{ByteLen, FilePath, FileStamp, ModTime, PeekBudget, Source};
use anyview_peek::{Body, PeekError, peek, probe};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use support::{Home, pane_budget, path};

/// Runs `probe` of `file` on a thread, so a probe that blocks fails the test instead of hanging it.
fn probe_within(file: FilePath) -> Result<anyview_peek::Probed, PeekError> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(probe(&file));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the probe returned")
}

fn fifo_in(dir: &Path) -> FilePath {
    let fifo = dir.join("pipe.txt");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success());
    FilePath::new(fifo).unwrap()
}

#[test]
fn a_fifo_is_refused_by_the_probe_instead_of_blocking_it() {
    let dir = tempfile::tempdir().unwrap();
    let error = probe_within(fifo_in(dir.path())).unwrap_err();
    assert!(matches!(error, PeekError::Unreadable { .. }), "{error:?}");
}

#[test]
fn a_device_behind_a_link_is_refused_by_the_probe() {
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("zero.png");
    std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
    let error = probe_within(FilePath::new(link).unwrap()).unwrap_err();
    assert!(matches!(error, PeekError::Unreadable { .. }), "{error:?}");
}

#[test]
fn a_peek_of_a_device_behind_a_link_reads_nothing() {
    let real = FilePath::new(path(Home::Image, "quadrants.png")).unwrap();
    let probed = probe(&real).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("zero.png");
    std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
    // The sniffed type of a picture, on a path that has since become a device.
    let source = Source::new(
        FilePath::new(link).unwrap(),
        FileStamp {
            len: ByteLen(1),
            modified: ModTime(0),
        },
    );
    let peeked = peek(&source, &probed.sniffed, &pane_budget());
    assert!(
        matches!(peeked.body, Body::Unavailable(_)),
        "{:?}",
        peeked.body
    );
}

#[test]
fn a_picture_over_the_byte_budget_is_not_read() {
    let real = FilePath::new(path(Home::Image, "quadrants.png")).unwrap();
    let probed = probe(&real).unwrap();
    let tight = PeekBudget {
        bytes: ByteLen(16),
        ..pane_budget()
    };
    let peeked = peek(&probed.source, &probed.sniffed, &tight);
    assert!(
        matches!(peeked.body, Body::Unavailable(_)),
        "{:?}",
        peeked.body
    );
}
