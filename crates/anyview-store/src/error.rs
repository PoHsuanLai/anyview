//! The one error `anyview-store` returns, with variants a caller acts on: a missing file is not
//! one (it is an empty store), a damaged file is told apart from a failing disk.

use std::path::PathBuf;

/// What the store was doing when a disk operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreOp {
    /// Reading a file.
    Read,
    /// Creating a directory.
    CreateDir,
    /// Writing the temporary file.
    Write,
    /// Syncing a file or directory to disk.
    Sync,
    /// Renaming the temporary file over the real one.
    Rename,
    /// Listing a directory.
    List,
    /// Deleting a file.
    Remove,
    /// Reading a file's size, mode or real location.
    Stat,
    /// Giving a file the mode of the one it replaces.
    Permissions,
}

/// Why the store could not read or write.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The disk refused an operation. A reader treats the store as unavailable; a writer leaves
    /// what is on disk untouched.
    #[error("{op:?} failed on {path:?}: {kind}")]
    Io {
        /// What was being done.
        op: StoreOp,
        /// The file or directory.
        path: PathBuf,
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// A file exists and is not what the store writes: cut short, edited by hand, or from a
    /// program that is not this one.
    #[error("{path:?} is not a valid store file: {reason}")]
    Corrupt {
        /// The file.
        path: PathBuf,
        /// The parser's description.
        reason: String,
    },
    /// No kept version has this id: it was pruned, or never kept.
    #[error("no kept version {id:?}")]
    NoSuchVersion {
        /// The id, as it was asked for.
        id: String,
    },
    /// A path that is not valid UTF-8 cannot be stored in JSON, so the file is not remembered.
    #[error("{path:?} is not valid UTF-8 and cannot be remembered")]
    PathNotUtf8 {
        /// The path.
        path: PathBuf,
    },
}
