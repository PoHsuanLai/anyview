//! One save at a time for each file in this process: two windows on the same file, or a save and
//! a revert, take turns, so one never renames over what the other is still writing.

use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, PoisonError};

/// The real paths being saved now.
static HELD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static RELEASED: Condvar = Condvar::new();

/// The right to write one file, held until it is dropped.
#[derive(Debug)]
pub(crate) struct SaveTurn {
    path: PathBuf,
}

impl SaveTurn {
    /// Waits for the turn on the real path `path`.
    pub(crate) fn wait_for(path: &Path) -> SaveTurn {
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        while held.iter().any(|taken| taken == path) {
            held = RELEASED.wait(held).unwrap_or_else(PoisonError::into_inner);
        }
        held.push(path.to_path_buf());
        SaveTurn {
            path: path.to_path_buf(),
        }
    }
}

impl Drop for SaveTurn {
    fn drop(&mut self) {
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        held.retain(|taken| *taken != self.path);
        RELEASED.notify_all();
    }
}
