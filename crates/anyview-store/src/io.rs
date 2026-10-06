//! The disk edge: whole-file reads, atomic writes, directory listing, removal and `stat`.
//! Nothing here decides anything; the pure modules say what to write.

use crate::error::{StoreError, StoreOp};
use anyview_core::{ByteLen, FileStamp, ModTime};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) fn io_error(op: StoreOp, path: &Path, error: &io::Error) -> StoreError {
    StoreError::Io {
        op,
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

/// The JSON in `path`, or `None` when the file (or a directory above it) does not exist.
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, StoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(StoreOp::Read, path, &error)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| StoreError::Corrupt {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
}

/// Writes `value` as the whole content of `path`: into `<name>.tmp` beside it, synced, renamed
/// over `path`, and the directory synced so the rename survives a crash. A concurrent reader
/// opens either the old file or the new one. One writer at a time: two would share the temporary
/// name. The directory is created when missing.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    write_json_as(path, value, 0o666)
}

/// [`write_json`] for a file only its owner may read (a kept version's sidecar).
pub(crate) fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    write_json_as(path, value, 0o600)
}

/// The folder `path` and every missing one above it, readable by its owner alone.
pub(crate) fn create_private_dir(path: &Path) -> Result<(), StoreError> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|e| io_error(StoreOp::CreateDir, path, &e))?;
    // A folder an older version made with the default mode is closed now.
    let _kept = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    Ok(())
}

fn write_json_as<T: Serialize>(path: &Path, value: &T, mode: u32) -> Result<(), StoreError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).map_err(|e| io_error(StoreOp::CreateDir, dir, &e))?;
    let mut bytes = serde_json::to_vec(value).map_err(|error| StoreError::Corrupt {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    bytes.push(b'\n');

    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    temp_name.push(".tmp");
    let temp = dir.join(temp_name);

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&temp)
        .map_err(|e| io_error(StoreOp::Write, &temp, &e))?;
    file.write_all(&bytes)
        .map_err(|e| io_error(StoreOp::Write, &temp, &e))?;
    file.sync_all()
        .map_err(|e| io_error(StoreOp::Sync, &temp, &e))?;
    drop(file);
    fs::rename(&temp, path).map_err(|e| io_error(StoreOp::Rename, path, &e))?;
    fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| io_error(StoreOp::Sync, dir, &e))
}

/// The `.json` files directly inside `dir`; none when the directory does not exist. The
/// temporary files of a write in progress end in `.tmp` and are not listed.
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
