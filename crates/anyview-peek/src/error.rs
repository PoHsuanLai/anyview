//! The crate's one error.

use anyview_archive::ArchiveError;
use anyview_core::{ByteLen, FormatKind};
use anyview_font::FontError;
use anyview_image::ImageError;
use anyview_text::TextError;
use ds::components::content::pdf_thumb::PdfTrouble;
use ds::prelude::Word;
use std::convert::Infallible;
use std::path::PathBuf;
use thiserror::Error;

/// Why a peek failed. Every peek reports through it, so the registry's one signature covers all of
/// them; the pane shows [`PeekError`]'s words as the reason a file has no preview.
#[derive(Debug, Error)]
pub enum PeekError {
    /// An image peek failed.
    #[error(transparent)]
    Image(#[from] ImageError),
    /// A text, code, Markdown, table or tree peek failed.
    #[error(transparent)]
    Text(#[from] TextError),
    /// An archive's listing failed.
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    /// A font could not be read.
    #[error(transparent)]
    Font(#[from] FontError),
    /// The PDF's first page could not be drawn.
    #[error("{}", .0.label())]
    Pdf(PdfTrouble),
    /// The folder could not be listed.
    #[error("cannot list {path:?}: {kind}")]
    Folder {
        /// The folder.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The path names nothing.
    #[error("{path:?} does not exist")]
    Missing {
        /// The path.
        path: PathBuf,
    },
    /// The path exists and cannot be read.
    #[error("cannot read {path:?}: {kind}")]
    Unreadable {
        /// The path.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The peek has to read more of the file than its budget allows.
    #[error("the file is {} bytes and the preview may read {}", .len.0, .allowed.0)]
    OverBudget {
        /// The file's length.
        len: ByteLen,
        /// The most the peek may read.
        allowed: ByteLen,
    },
    /// The file's sniffed kind belongs to another peek.
    #[error("a {kind:?} file is not what this peek reads")]
    WrongKind {
        /// What the file was sniffed as.
        kind: FormatKind,
    },
}

/// A peek that cannot fail names `Infallible`; this is the one conversion the registry needs.
impl From<Infallible> for PeekError {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}
