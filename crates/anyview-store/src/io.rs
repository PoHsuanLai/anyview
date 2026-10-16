//! The disk edge: whole-file reads, atomic writes, directory listing, removal and `stat`.
//! Nothing here decides anything; the pure modules say what to write.

use crate::error::{StoreError, StoreOp};
use crate::sys;
use anyview_core::{ByteLen, FileStamp, ModTime};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn io_error(op: StoreOp, path: &Path, error: &io::Error) -> StoreError {
    StoreError::Io {
        op,
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

/// The most a store file may be: the history holds a few hundred paths and a record one, so a
/// file over this is not one the store wrote.
const STORE_FILE_CAP: ds_helpers::ReadCap = ds_helpers::ReadCap::mib(16);

/// The JSON in `path`, or `None` when the file (or a directory above it) does not exist.
/// Anything else that keeps it from being read is an error, never "absent": a FIFO or a folder at
/// the path, or a refused read, is `Io` (left as it is), and a file over [`STORE_FILE_CAP`] is
/// `Corrupt` (a writer moves it aside before writing a fresh one).
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, StoreError> {
    let bytes = match ds_helpers::read_capped(path, STORE_FILE_CAP) {
        Ok(bytes) => bytes,
        Err(error) => return read_failed(path, error),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| StoreError::Corrupt {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
}

// `ReadError` is `non_exhaustive`: a failure this crate cannot name yet is a refused read, so
// the file is left as it is.
#[allow(clippy::wildcard_enum_match_arm)]
fn read_failed<T>(path: &Path, error: ds_helpers::ReadError) -> Result<Option<T>, StoreError> {
    match error {
        ds_helpers::ReadError::NotFound { .. } => Ok(None),
        ds_helpers::ReadError::NotRegular { .. } => Err(io_error(
            StoreOp::Read,
            path,
            &io::Error::from(io::ErrorKind::InvalidInput),
        )),
        ds_helpers::ReadError::TooBig { .. } => Err(StoreError::Corrupt {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }),
        ds_helpers::ReadError::Io { source, .. } => Err(io_error(StoreOp::Read, path, &source)),
        other => Err(io_error(StoreOp::Read, path, &io::Error::other(other))),
    }
}

/// Writes `value` as the whole content of `path` with quire's `ds_helpers::write_atomic`: a
/// uniquely named temporary file beside it, synced, renamed over `path`, and the directory synced
/// so the rename survives a crash. A concurrent reader opens either the old file or the new one,
/// and two writers in this process take turns. A new file is its owner's alone; the directory is
/// created when missing, closed to everyone else: the history lists every file the person opened.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    create_private_dir(dir)?;
    let mut bytes = serde_json::to_vec(value).map_err(|error| StoreError::Corrupt {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    bytes.push(b'\n');
    ds_helpers::write_atomic(path, &bytes, ds_helpers::NewFileMode::OwnerOnly)
        .map_err(|error| io_error(StoreOp::Write, &error.path, &error.source))
}

/// Moves the damaged store file at `path` aside, to `<name>.bad-<at>` beside it, so a fresh one
/// can be written without losing the person's bytes. Returns where it went.
pub(crate) fn move_aside(path: &Path, at: u64) -> Result<PathBuf, StoreError> {
    ds_helpers::move_aside(path, ds_helpers::UnixSeconds(at))
        .map_err(|error| io_error(StoreOp::Rename, &error.path, &error.source))
}

/// The folder `path` and every missing one above it, readable by its owner alone.
pub(crate) fn create_private_dir(path: &Path) -> Result<(), StoreError> {
    sys::create_private_dir(path).map_err(|e| io_error(StoreOp::CreateDir, path, &e))
}

/// The `.json` files directly inside `dir`; none when the directory does not exist. The
/// temporary files of a write in progress end in `.tmp`, and a damaged file moved aside ends in
/// `.bad-<seconds>`, so neither is listed.
pub(crate) fn list_json(dir: &Path) -> Result<Vec<PathBuf>, StoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(StoreOp::List, dir, &error)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| io_error(StoreOp::List, dir, &e))?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Deletes `path`; a file already gone is fine.
pub(crate) fn remove(path: &Path) -> Result<(), StoreError> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(io_error(StoreOp::Remove, path, &error))
        }
        Ok(()) | Err(_) => Ok(()),
    }
}

/// The length and modification time of `path` now, or `None` when it cannot be read (gone, or
/// not reachable): such a file has no memory worth keeping.
pub(crate) fn stamp_of(path: &Path) -> Option<FileStamp> {
    let meta = fs::metadata(path).ok()?;
    Some(FileStamp {
        len: ByteLen(meta.len()),
        modified: ModTime::from_system_time(meta.modified().ok()?),
    })
}
