//! One save at a time for each file in this process: two windows on the same file, or a save and
//! a revert, take turns, so one never renames over what the other is still writing.
//!
//! And one writer at a time for a store, across processes: the viewer and a program embedding it
//! (a terminal with a viewer pane) may both record a view, so a writer holds an advisory `flock`
//! on `<root>/.lock` for its whole read, merge and replace.

use crate::error::{StoreError, StoreOp};
use crate::io::io_error;
use rustix::fs::FlockOperation;
use rustix::io::Errno;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
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

/// The name of the lock file inside a store root.
const LOCK_FILE: &str = ".lock";

/// The right to write one store, across every process, held until it is dropped. The lock is
/// advisory `flock(2)` on an open file, so the system releases it when the holder exits, however
/// it exits: a crashed writer never leaves the store locked. It also excludes a second
/// [`StoreLock`] taken in this process, because `flock` belongs to the open file, not the process.
#[derive(Debug)]
pub(crate) struct StoreLock {
    /// Closing the file releases the lock.
    _file: File,
}

impl StoreLock {
    /// Waits for the exclusive lock on the store at `root`, creating the folder and the lock file
    /// when missing. Blocking.
    pub(crate) fn exclusive(root: &Path) -> Result<StoreLock, StoreError> {
        fs::create_dir_all(root).map_err(|e| io_error(StoreOp::CreateDir, root, &e))?;
        let path = root.join(LOCK_FILE);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .map_err(|e| io_error(StoreOp::Lock, &path, &e))?;
        loop {
            match rustix::fs::flock(&file, FlockOperation::LockExclusive) {
                Ok(()) => return Ok(StoreLock { _file: file }),
                Err(Errno::INTR) => {}
                Err(errno) => return Err(io_error(StoreOp::Lock, &path, &errno.into())),
            }
        }
    }
}
