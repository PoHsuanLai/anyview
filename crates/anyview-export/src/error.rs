//! The one error `anyview-export` returns.

use anyview_core::CoreError;
use anyview_image::ImageError;
use anyview_pdf::PdfError;
use anyview_text::TextError;
use std::path::PathBuf;

/// Why an export or a printout could not be made. Whatever it is, nothing partial is left at
/// the destination: a file is written whole or not at all, and an export that fails after some
/// of its files were written removes them.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The image could not be read, decoded or encoded.
    #[error("{0}")]
    Image(#[from] ImageError),
    /// The PDF could not be read, drawn or written.
    #[error("{0}")]
    Pdf(#[from] PdfError),
    /// The text could not be read or laid out.
    #[error("{0}")]
    Text(#[from] TextError),
    /// The page could not be printed to a PDF.
    #[error("cannot lay the page out: {0}")]
    Layout(#[from] ds_blitz::PdfError),
    /// A path was not a file path.
    #[error("{0}")]
    Path(#[from] CoreError),
    /// The disk refused a read.
    #[error("cannot read {path:?}: {kind}")]
    Read {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The disk refused a write.
    #[error("cannot write {path:?}: {kind}")]
    Write {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The destination appeared while the file was being made, so it is left as it is.
    #[error("{path:?} already exists")]
    Exists {
        /// The file.
        path: PathBuf,
    },
    /// No free name beside the original.
    #[error("no free name beside {path:?}")]
    NoFreeName {
        /// The original.
        path: PathBuf,
    },
    /// The choice has no job for this file: its kind is not what the choice exports.
    #[error("nothing to write for this file")]
    NothingToWrite,
    /// A job that a plugin or the player carries out, not this crate.
    #[error("a recording's export is not written here")]
    NotADocument,
}
