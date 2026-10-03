//! Shared by the store's integration tests: real scratch files and their fingerprints.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used, dead_code)]

use anyview_core::{ByteLen, FilePath, FileStamp, ModTime};
use std::fs;
use std::path::Path;

/// A file named `name` in `dir` holding `body`, and the stamp it has now.
pub fn viewed_file(dir: &Path, name: &str, body: &str) -> (FilePath, FileStamp) {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    (FilePath::new(&path).unwrap(), stamp_of(&path))
}

pub fn stamp_of(path: &Path) -> FileStamp {
    let meta = fs::metadata(path).unwrap();
    FileStamp {
        len: ByteLen(meta.len()),
        modified: ModTime::from_system_time(meta.modified().unwrap()),
    }
}

/// Rewrites `path` with `body` and a modification time one minute ahead of what it had, so the
/// fingerprint differs whatever the file system's timestamp resolution.
pub fn replace_file(path: &FilePath, body: &str) {
    let before = fs::metadata(path.as_path()).unwrap().modified().unwrap();
    fs::write(path.as_path(), body).unwrap();
    let file = fs::File::options()
        .write(true)
        .open(path.as_path())
        .unwrap();
    file.set_modified(before + std::time::Duration::from_secs(60))
        .unwrap();
}
