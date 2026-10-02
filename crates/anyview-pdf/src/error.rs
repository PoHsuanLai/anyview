//! The one error `anyview-pdf` returns.

use anyview_core::{CoreError, EditKind, PageIndex};

/// Why a PDF could not be opened, read, drawn, edited or written.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// The document is encrypted and the password given does not open it (or none was given).
    #[error("the PDF needs a password")]
    Locked,
    /// pdfrum could not open, read or draw something.
    #[error("{0}")]
    Pdf(#[source] pdfrum::Error),
    /// pdfrum's editor refused an edit or a write.
    #[error("cannot edit the PDF: {0}")]
    Edit(#[from] pdfrum_edit::Error),
    /// A value handed in at the boundary named nothing the viewer can hold.
    #[error("{0}")]
    Core(#[from] CoreError),
    /// A file could not be read.
    #[error("cannot read the file: {0}")]
    Io(#[from] std::io::Error),
    /// A document with no pages has nothing to show.
    #[error("the PDF has no pages")]
    NoPages,
    /// A page the document does not have.
    #[error("page {page:?} is not in a document of {count} pages")]
    PageOutOfRange {
        /// The page asked for.
        page: PageIndex,
        /// How many pages the document has.
        count: u32,
    },
    /// An edit that would leave the document without pages.
    #[error("that would delete every page")]
    WouldDeleteAll,
    /// An edit a PDF does not take.
    #[error("a PDF has no {kind:?} edit")]
    EditUnsupported {
        /// The sort of edit that was asked for.
        kind: EditKind,
    },
    /// The work was stopped before it finished.
    #[error("stopped")]
    Stopped,
}

impl From<pdfrum::Error> for PdfError {
    fn from(error: pdfrum::Error) -> Self {
        // pdfrum's error enum is non-exhaustive, so only the one case worth a name is matched.
        if matches!(error, pdfrum::Error::WrongPassword) {
            PdfError::Locked
        } else {
            PdfError::Pdf(error)
        }
    }
}
