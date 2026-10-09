//! A book bound as a PDF. An EPUB's chapters are laid out one at a time on a fixed reading page
//! (`epub`) and a comic's pictures are put one to a page (`comic`); the pieces are bound into one
//! document with the book's contents as its outline, and the PDF stage shows that, so a book has
//! the page stack, find, zoom, thumbnails, resume and export of any PDF. The bound document is kept
//! for the session (`kept`), keyed by the file's stamp, so opening it again is instant.

mod comic;
mod epub;
mod kept;

use crate::io::OpenError;
use anyview_book::BookError;
use anyview_core::{BookFormat, Facts, FormatDetail, Sniffed, Source};
use anyview_pdf::PdfDocument;

/// The document a book is bound as, and the rows of its Info tab.
pub(super) struct Bound {
    pub(super) document: PdfDocument,
    pub(super) facts: Facts,
}

/// Bind the book `src` (an EPUB or a comic zip). Blocking: reads the book and lays it out, unless
/// the session has bound this version of it already.
pub(super) fn bind(src: &Source, sniffed: &Sniffed) -> Result<Bound, OpenError> {
    let FormatDetail::Book(format) = sniffed.detail() else {
        return Err(OpenError::Unrecognised);
    };
    let (bytes, facts) = match kept::get(src) {
        Some(found) => found,
        None => {
            // The book's own rows only: the General section (kind, size, dates) is the window's.
            let base = Facts::empty();
            let (bytes, facts) = match format {
                BookFormat::Epub => epub::bind(src.path(), base)?,
                BookFormat::Cbz => comic::bind(src.path(), base)?,
            };
            (kept::put(src, bytes, &facts), facts)
        }
    };
    let document = PdfDocument::from_bytes(bytes.to_vec()).map_err(|_| BookError::Empty)?;
    Ok(Bound { document, facts })
}
