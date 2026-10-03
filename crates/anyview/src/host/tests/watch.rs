//! The watcher against a real folder: what an editor does to a file reaches the window once.

use crate::host::Watcher;
use anyview_core::FilePath;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// Short, so a test waits little; the burst logic is the same at any length.
const SETTLE: Duration = Duration::from_millis(60);
const PATIENCE: Duration = Duration::from_secs(5);

fn told_by(watcher: &Watcher) -> (crate::host::WindowWatch, Receiver<FilePath>) {
    let (send, receive) = channel();
    let watch = watcher.window(move |file| {
        let _gone = send.send(file);
    });
    (watch, receive)
}

fn file(path: &Path) -> FilePath {
    FilePath::new(path).unwrap()
}

#[test]
fn a_write_to_the_watched_file_is_told_once_however_many_events_it_made() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(dir.path()).unwrap().join("a.txt");
    std::fs::write(&path, "one").unwrap();
    let watcher = Watcher::start(SETTLE).unwrap();
    let (watch, told) = told_by(&watcher);
    watch.watch(&file(&path)).unwrap();

    // An editor writes twice in a row.
    std::fs::write(&path, "two").unwrap();
    std::fs::write(&path, "three").unwrap();

    assert_eq!(told.recv_timeout(PATIENCE).unwrap(), file(&path));
    assert!(
        told.recv_timeout(SETTLE * 4).is_err(),
        "the burst was one change"
    );
}

#[test]
fn a_file_renamed_over_the_watched_one_is_a_change_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(dir.path()).unwrap();
    let path = folder.join("a.txt");
    std::fs::write(&path, "one").unwrap();
    let watcher = Watcher::start(SETTLE).unwrap();
    let (watch, told) = told_by(&watcher);
    watch.watch(&file(&path)).unwrap();

    std::fs::write(folder.join(".a.txt.tmp"), "two").unwrap();
    std::fs::rename(folder.join(".a.txt.tmp"), &path).unwrap();

    assert_eq!(told.recv_timeout(PATIENCE).unwrap(), file(&path));
}

#[test]
fn the_neighbours_of_the_file_and_a_file_given_up_tell_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(dir.path()).unwrap();
    let (watched, beside, other) = (
        folder.join("a.txt"),
        folder.join("b.txt"),
        folder.join("c.txt"),
    );
    for path in [&watched, &beside, &other] {
        std::fs::write(path, "x").unwrap();
    }
    let watcher = Watcher::start(SETTLE).unwrap();
    let (watch, told) = told_by(&watcher);
    watch.watch(&file(&watched)).unwrap();
    // Another window keeps the folder watched, so the events still come.
    let (_second, _unheard) = told_by(&watcher);
    _second.watch(&file(&other)).unwrap();

    std::fs::write(&beside, "y").unwrap();
    assert!(
        told.recv_timeout(SETTLE * 6).is_err(),
        "a neighbour is not the file"
    );

    watch.watch(&file(&beside)).unwrap();
    std::fs::write(&watched, "z").unwrap();
    assert!(
        told.recv_timeout(SETTLE * 6).is_err(),
        "the window moved on from the file"
    );
    std::fs::write(&beside, "w").unwrap();
    assert_eq!(told.recv_timeout(PATIENCE).unwrap(), file(&beside));

    watch.unwatch();
    std::fs::write(&beside, "v").unwrap();
    assert!(
        told.recv_timeout(SETTLE * 6).is_err(),
        "a window that watches nothing hears nothing"
    );
}
