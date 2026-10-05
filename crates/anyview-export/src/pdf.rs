//! The jobs that write a PDF, and the text of one.

use crate::error::ExportError;
use crate::pictures::page_picture;
use crate::session::Session;
use anyview_core::{FilePath, NonEmpty, PageSelection, TextFlavour};

/// The pages of the PDF `file` that `pages` selects, as a PDF.
pub(crate) fn pages(
    session: &mut Session,
    file: &FilePath,
    pages: PageSelection,
) -> Result<Vec<u8>, ExportError> {
    let doc = session.document(file)?;
    Ok(anyview_pdf::write_pages(&doc, pages)?)
}

/// A PDF with each of `files` on a page of its own.
pub(crate) fn of_images(files: &NonEmpty<FilePath>) -> Result<Vec<u8>, ExportError> {
    let pictures = files
        .iter()
        .map(page_picture)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(anyview_pdf::pdf_of_pictures(&pictures)?)
}

/// The text of the pages of the PDF `file` that `pages` selects.
pub(crate) fn text(
    session: &mut Session,
    file: &FilePath,
    pages: PageSelection,
    flavour: TextFlavour,
) -> Result<Vec<u8>, ExportError> {
    let doc = session.document(file)?;
    Ok(anyview_pdf::write_text(&doc, pages, flavour)?.into_bytes())
}
