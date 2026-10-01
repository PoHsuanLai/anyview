//! The write API: what the viewer links. Reads, decides with the pure modules, writes whole files.
//!
//! One `StoreWriter` per store root at a time (the viewer is a single instance): a read-modify-
//! write of the history would otherwise lose one of two concurrent updates. Blocking; call it
//! from a worker.

use crate::error::StoreError;
use crate::history::{History, HistoryCap, HistoryEntry, history_after_view};
use crate::io;
use crate::label::resume_label;
use crate::reader::{HISTORY_FILE, HistoryRead, read_history};
use crate::record::{Prune, ResumeRecord, applicable, prune_decision, resume_file_name};
use crate::viewed::Viewed;
use anyview_core::{FilePath, FileStamp, FormatKind, Resume};
use std::path::PathBuf;

/// The directory inside a store root that holds one file of view memory per viewed file.
const RESUME_DIR: &str = "resume";

/// The result of recording a view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRecorded {
    /// Set when the history file on disk was damaged and was replaced by a fresh one: the
    /// damage, for the caller's log. `None` when the history was read normally or was absent.
    pub replaced_damaged: Option<StoreError>,
}

/// Writes the store at one root.
#[derive(Debug, Clone)]
pub struct StoreWriter {
    root: PathBuf,
    cap: HistoryCap,
}

impl StoreWriter {
    /// A writer for the store at `root` (created on first write) that keeps `cap` files.
    pub fn new(root: impl Into<PathBuf>, cap: HistoryCap) -> StoreWriter {
        StoreWriter {
            root: root.into(),
            cap,
        }
    }

    /// Notes that `path` was viewed at `viewed` and left at `resume`: it goes first in the
    /// history with a label derived from `resume`. Then the view memory of files that are gone,
    /// replaced, or no longer in the history is deleted, so it is the one write that prunes.
    ///
    /// A damaged `history.json` is replaced and reported in the result; a disk failure leaves the
    /// file as it was and is the error. A failure while pruning comes after the history was
    /// written.
    pub fn record_view(
        &self,
        path: &FilePath,
        kind: FormatKind,
        viewed: Viewed,
        resume: &Resume,
    ) -> Result<ViewRecorded, StoreError> {
        resume_file_name(path)?; // refuses a path JSON cannot hold, before anything is written
        let (before, replaced_damaged) = match read_history(&self.root) {
            HistoryRead::Loaded(history) => (history, None),
            HistoryRead::Absent => (History::default(), None),
            HistoryRead::Unavailable(error @ StoreError::Corrupt { .. }) => {
                (History::default(), Some(error))
            }
            HistoryRead::Unavailable(
                error @ (StoreError::Io { .. } | StoreError::PathNotUtf8 { .. }),
            ) => {
                return Err(error);
            }
        };
        let entry = HistoryEntry {
            path: path.clone(),
            kind,
            viewed,
            label: resume_label(resume),
        };
        let after = history_after_view(&before, entry, self.cap);
        io::write_json(&self.root.join(HISTORY_FILE), &after)?;
        self.prune_resume(&after)?;
        Ok(ViewRecorded { replaced_damaged })
    }

    /// Remembers `resume` for `path`, whose file has `stamp` now (the stamp read when the file
    /// was opened). `Resume::Nothing` forgets instead. This does not touch the history: its
    /// label comes from [`StoreWriter::record_view`].
    pub fn save_resume(
        &self,
        path: &FilePath,
        stamp: FileStamp,
        resume: &Resume,
    ) -> Result<(), StoreError> {
        let file = self.resume_path(path)?;
        if matches!(resume, Resume::Nothing) {
            return io::remove(&file);
        }
        let record = ResumeRecord {
            path: path.clone(),
            stamp,
            resume: resume.clone(),
        };
        io::write_json(&file, &record)
    }

    /// What was remembered for `path` when its file looked like `current`: `None` when nothing
    /// was, or the file was edited or replaced since. A damaged record is the error; treat it
    /// as nothing remembered.
    pub fn load_resume(
        &self,
        path: &FilePath,
        current: FileStamp,
    ) -> Result<Option<Resume>, StoreError> {
        let record = io::read_json::<ResumeRecord>(&self.resume_path(path)?)?;
        Ok(record.and_then(|record| applicable(record, path, current)))
    }

    fn resume_path(&self, path: &FilePath) -> Result<PathBuf, StoreError> {
        Ok(self.resume_dir().join(resume_file_name(path)?))
    }

    fn resume_dir(&self) -> PathBuf {
        self.root.join(RESUME_DIR)
    }

    /// Deletes every record `prune_decision` removes, and every record that does not parse.
    fn prune_resume(&self, history: &History) -> Result<(), StoreError> {
        for file in io::list_json(&self.resume_dir())? {
            match io::read_json::<ResumeRecord>(&file) {
                Ok(Some(record)) => {
                    let current = io::stamp_of(record.path.as_path());
                    match prune_decision(&record, history, current) {
                        Prune::Keep => {}
                        Prune::Remove(_) => io::remove(&file)?,
                    }
                }
                Ok(None) => {}
                Err(StoreError::Corrupt { .. }) => io::remove(&file)?,
                Err(error @ (StoreError::Io { .. } | StoreError::PathNotUtf8 { .. })) => {
                    return Err(error);
                }
            }
        }
        Ok(())
    }
}
