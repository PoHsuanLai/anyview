//! The read API: what the launcher links. It never locks, never writes, and never fails: a store
//! that is missing, empty or damaged reads as an empty history with the reason alongside.

use crate::error::StoreError;
use crate::history::{History, HistoryEntry};
use crate::io;
use std::path::Path;

/// The file inside a store root that holds the history.
pub(crate) const HISTORY_FILE: &str = "history.json";

/// What reading the history found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryRead {
    /// The history, as last written.
    Loaded(History),
    /// There is no history yet: the root or the file does not exist. Not an error.
    Absent,
    /// The file is there and cannot be used; treat the history as empty and report the reason.
    Unavailable(StoreError),
}

impl HistoryRead {
    /// The files to offer, newest first; empty unless the history loaded.
    pub fn entries(&self) -> &[HistoryEntry] {
        match self {
            HistoryRead::Loaded(history) => &history.entries,
            HistoryRead::Absent | HistoryRead::Unavailable(_) => &[],
        }
    }

    /// Why the history is unavailable, for the caller's log.
    pub fn fault(&self) -> Option<&StoreError> {
        match self {
            HistoryRead::Unavailable(error) => Some(error),
            HistoryRead::Loaded(_) | HistoryRead::Absent => None,
        }
    }
}

/// Reads the history in the store at `root`. Safe while the viewer writes: the file is replaced
/// by rename, so this sees the old history or the new one, never a mixture.
#[must_use]
pub fn read_history(root: &Path) -> HistoryRead {
    match io::read_json::<History>(&root.join(HISTORY_FILE)) {
        Ok(Some(history)) => HistoryRead::Loaded(history),
        Ok(None) => HistoryRead::Absent,
        Err(error) => HistoryRead::Unavailable(error),
    }
}
