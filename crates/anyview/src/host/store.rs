//! The viewer's memory on disk, written from whichever worker has something to keep. The store
//! has one writer (anyview-store), so every window of the process goes through this one lock.

use anyview_core::{FilePath, FileStamp, FormatKind, Resume};
use anyview_store::{Clock, HistoryCap, SavedAt, StoreError, StoreWriter};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

/// The store of recently viewed files and per-file view memory.
pub struct Store {
    writer: Mutex<StoreWriter>,
    now: Clock,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// A store under `root`, stamping each view with `now()`.
    #[must_use]
    pub fn new(root: &Path, now: Clock) -> Store {
        Store {
            writer: Mutex::new(StoreWriter::new(root, HistoryCap::DEFAULT)),
            now,
        }
    }

    /// The time as the program's clock reads it now, for a version kept.
    #[must_use]
    pub fn saved_at(&self) -> SavedAt {
        SavedAt((self.now)().0)
    }

    /// `path` was shown. Blocking.
    pub fn viewed(&self, path: &FilePath, kind: FormatKind) -> Result<(), StoreError> {
        let writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        writer
            .record_view(path, kind, (self.now)(), &Resume::Nothing)
            .map(drop)
    }

    /// Keep where the person is in `path`. Blocking.
    pub fn remember(
        &self,
        path: &FilePath,
        stamp: FileStamp,
        resume: &Resume,
    ) -> Result<(), StoreError> {
        let writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        writer.save_resume(path, stamp, resume)
    }

    /// Where the person left `path`, if the file is as it was then. Blocking.
    pub fn resume(&self, path: &FilePath, stamp: FileStamp) -> Result<Option<Resume>, StoreError> {
        let writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        writer.load_resume(path, stamp)
    }

    /// The width the person left the left panel at, if one was kept. Blocking.
    pub fn panel_width(&self) -> Result<Option<u32>, StoreError> {
        let writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        writer.load_panel_width()
    }

    /// Keep the width the person left the left panel at. Blocking.
    pub fn keep_panel_width(&self, width: u32) -> Result<(), StoreError> {
        let writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        writer.save_panel_width(width)
    }
}
