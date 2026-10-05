//! The one error `anyview-book` returns, with variants a caller acts on.

use anyview_archive::ArchiveError;
use std::io::ErrorKind;

/// Why a book could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BookError {
    /// The zip could not be listed or one of its entries read.
    #[error("{0}")]
    Archive(#[from] ArchiveError),
    /// A part of the package is not well-formed XML.
    #[error("{part} is not well-formed: {reason}")]
    Xml {
        /// Which part.
        part: &'static str,
        /// The parser's own words.
        reason: String,
    },
    /// A part the package must name is missing from it.
    #[error("the package has no {0}")]
    Missing(&'static str),
    /// The book has nothing to read: no chapter, no page.
    #[error("the book has nothing to read")]
    Empty,
    /// The section asked for is past the end.
    #[error("the book has no such section")]
    NoSuchSection,
}

impl BookError {
    /// What the disk said, when the file could not be read at all.
    pub fn read_error(&self) -> Option<ErrorKind> {
        match self {
            BookError::Archive(ArchiveError::Read { kind, .. }) => Some(*kind),
            BookError::Archive(_)
            | BookError::Xml { .. }
            | BookError::Missing(_)
            | BookError::Empty
            | BookError::NoSuchSection => None,
        }
    }

    /// Whether the book is encrypted, which nothing here asks a password for.
    pub fn is_locked(&self) -> bool {
        matches!(self, BookError::Archive(ArchiveError::Encrypted))
    }
}
