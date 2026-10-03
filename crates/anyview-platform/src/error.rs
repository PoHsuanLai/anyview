//! The one error `anyview-platform` returns, with variants a caller acts on: no bus means the
//! feature is off, a failed call is reported, a bad file is skipped.

use std::path::PathBuf;

/// What the edge was doing to a file or directory when the disk refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOp {
    /// Reading a file.
    Read,
    /// Creating a directory.
    CreateDir,
    /// Writing a file.
    Write,
    /// Renaming a file over another.
    Rename,
}

/// Why a platform call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// The environment names no session bus, so nothing that needs one can run.
    #[error("there is no session bus")]
    NoBus,
    /// A D-Bus call, registration or reply failed. A caller reports it and carries on without
    /// the feature.
    #[error("{call} failed on the bus: {reason}")]
    Bus {
        /// What was being done: a method, a name or an object path.
        call: &'static str,
        /// What the bus library reported.
        reason: String,
    },
    /// The disk refused an operation.
    #[error("{op:?} failed on {path:?}: {kind}")]
    Io {
        /// What was being done.
        op: IoOp,
        /// The file or directory.
        path: PathBuf,
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// A thumbnail file or the pixels offered for one cannot be a PNG.
    #[error("{path:?} is not a usable thumbnail: {reason}")]
    Thumbnail {
        /// The thumbnail file, or the file it would be for.
        path: PathBuf,
        /// The PNG codec's description.
        reason: String,
    },
    /// A program could not be started.
    #[error("cannot start {program:?}: {kind}")]
    Spawn {
        /// The program as named.
        program: String,
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// A desktop entry's `Exec` line cannot become a command.
    #[error("the entry {id:?} has no usable Exec line: {reason}")]
    Exec {
        /// The entry's id.
        id: String,
        /// The parser's description.
        reason: String,
    },
}

impl PlatformError {
    /// A failed bus call named `call`.
    pub(crate) fn bus(call: &'static str, error: impl std::fmt::Display) -> Self {
        PlatformError::Bus {
            call,
            reason: error.to_string(),
        }
    }

    /// A failed disk operation.
    pub(crate) fn io(op: IoOp, path: impl Into<PathBuf>, error: &std::io::Error) -> Self {
        PlatformError::Io {
            op,
            path: path.into(),
            kind: error.kind(),
        }
    }
}
