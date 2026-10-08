//! The one error `anyview-archive` returns, with variants a caller acts on.

use anyview_core::{ArchiveFormat, ByteLen, Input, ReadAtStream};
use std::path::PathBuf;

/// Why an archive could not be listed or one of its entries read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveError {
    /// The disk refused to read the file.
    #[error("cannot read {path:?}: {kind}")]
    Read {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The file's sniffed kind belongs to another peek.
    #[error("a {kind:?} file is not what this peek reads")]
    WrongKind {
        /// What the file was sniffed as.
        kind: anyview_core::FormatKind,
    },
    /// A budget that allows no byte.
    #[error("the peek budget allows no bytes")]
    NoBudget,
    /// The archive's index is larger than the bytes a listing may read.
    #[error("the archive's index is larger than the {} bytes the preview may read", .allowed.0)]
    OverBudget {
        /// The most the listing may read.
        allowed: ByteLen,
    },
    /// The archive does not parse as its format.
    #[error("not a valid {} archive: {reason}", crate::container::format_name(*.format))]
    Malformed {
        /// What the file was sniffed as.
        format: ArchiveFormat,
        /// The parser's own words.
        reason: String,
    },
    /// An entry is encrypted, and nothing here asks for a password.
    #[error("the entry is encrypted")]
    Encrypted,
    /// The archive has no entry with that path.
    #[error("no entry {path:?} in the archive")]
    NoSuchEntry {
        /// What was asked for.
        path: String,
    },
    /// The entry is larger than the caller allowed, or lies beyond the part of a compressed
    /// stream the caller allowed to be unpacked.
    #[error("the entry is larger than the {} bytes allowed", .allowed.0)]
    TooLarge {
        /// What the caller allowed.
        allowed: ByteLen,
    },
    /// The entry is not a file (a folder or a link) and has no bytes to extract.
    #[error("the entry is not a file")]
    NotAFile,
}

impl ArchiveError {
    pub(crate) fn read(src: &Input, error: &std::io::Error) -> Self {
        ArchiveError::Read {
            path: src.label(),
            kind: error.kind(),
        }
    }

    /// A stream over `src` at its start, once its first byte has read: a missing file, a FIFO or
    /// a failing source is a [`ArchiveError::Read`] here, not a malformed archive later.
    pub(crate) fn open(src: &Input) -> Result<ReadAtStream, Self> {
        src.bytes()
            .read_at(0, &mut [0u8; 1])
            .map_err(|e| ArchiveError::read(src, &e))?;
        Ok(src.reader())
    }

    pub(crate) fn malformed(format: ArchiveFormat, reason: impl std::fmt::Display) -> Self {
        ArchiveError::Malformed {
            format,
            reason: reason.to_string(),
        }
    }
}
