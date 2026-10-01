//! The disk edge: whole-file reads, atomic writes, directory listing, removal and `stat`.
//! Nothing here decides anything; the pure modules say what to write.

use crate::error::{StoreError, StoreOp};
use serde::de::DeserializeOwned;
use std::fs;
use std::io;
use std::path::Path;

fn io_error(op: StoreOp, path: &Path, error: &io::Error) -> StoreError {
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
