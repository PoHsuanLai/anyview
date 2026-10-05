//! The save pipeline as three types: [`Pending`] bytes, [`BackedUp`] bytes whose original is in
//! the versions store, and [`Written`]. [`BackedUp::write_in_place`] is the one function in the
//! program that writes over a person's file, and a `BackedUp` can only be made by
//! [`Versions::back_up`](crate::Versions::back_up), so an in-place save without the original
//! kept does not compile.

use crate::error::{StoreError, StoreOp};
use crate::io::io_error;
use crate::versions::VersionId;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The bytes an edit produced for `target`, not yet saved. Applying an edit is the caller's work;
/// this is where its result waits for the original to be kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub(crate) target: PathBuf,
    pub(crate) bytes: Vec<u8>,
}

impl Pending {
    /// `bytes` as the new whole content of the existing file `target`.
    pub fn new(target: impl Into<PathBuf>, bytes: Vec<u8>) -> Pending {
        Pending {
            target: target.into(),
            bytes,
        }
    }
}

/// A pending save whose original is already kept as `kept`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackedUp {
    target: PathBuf,
    bytes: Vec<u8>,
    kept: VersionId,
    mode: fs::Permissions,
}

impl BackedUp {
    /// Only the versions store makes one, after the original is safely copied.
    pub(crate) fn new(
        target: PathBuf,
        bytes: Vec<u8>,
        kept: VersionId,
        mode: fs::Permissions,
    ) -> BackedUp {
        BackedUp {
            target,
            bytes,
            kept,
            mode,
        }
    }

    /// The version that holds the original.
    pub fn kept(&self) -> &VersionId {
        &self.kept
    }

    /// Replaces the original with the new bytes, whole or not at all: they go into a temporary
    /// file beside it with the original's mode, are synced, and are renamed over it, so a crash
    /// or a full disk leaves the original as it was. Calling it again writes the same bytes
    /// again, which is how a caller retries after an error.
    pub fn write_in_place(&self) -> Result<Written, StoreError> {
        let dir = self.target.parent().unwrap_or_else(|| Path::new("."));
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(self.target.file_name().unwrap_or_default());
        temp_name.push(format!(".anyview-{}.tmp", std::process::id()));
        let temp = dir.join(temp_name);

        let result = self.write_temp(&temp).and_then(|()| {
            fs::rename(&temp, &self.target).map_err(|e| io_error(StoreOp::Rename, &self.target, &e))
        });
        if let Err(error) = result {
            // The temporary file is ours alone; the original was never opened for writing.
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        fs::File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(|e| io_error(StoreOp::Sync, dir, &e))?;
        Ok(Written {
            target: self.target.clone(),
            kept: self.kept.clone(),
        })
    }

    fn write_temp(&self, temp: &Path) -> Result<(), StoreError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(temp)
            .map_err(|e| io_error(StoreOp::Write, temp, &e))?;
        file.set_permissions(self.mode.clone())
            .map_err(|e| io_error(StoreOp::Permissions, temp, &e))?;
        file.write_all(&self.bytes)
            .map_err(|e| io_error(StoreOp::Write, temp, &e))?;
        file.sync_all()
            .map_err(|e| io_error(StoreOp::Sync, temp, &e))
    }
}

/// A file that now holds the saved bytes, and the version that holds what it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The file that was replaced.
    pub target: PathBuf,
    /// Its previous content, restorable through [`Versions::restore`](crate::Versions::restore).
    pub kept: VersionId,
}
