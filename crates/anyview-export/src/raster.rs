//! The jobs that encode pixels: an image file resized, or a page of a PDF drawn.

use crate::error::ExportError;
use crate::session::Session;
use anyview_core::{Dpi, FilePath, MetadataCarry, PageIndex, RasterTarget, Resize};
use anyview_image::Rgba8;
use anyview_pdf::{Backend, PdfBackend, PdfDone, PdfJob, Stop, Ticket};

/// The image `file`, resized and encoded.
pub(crate) fn image(
    file: &FilePath,
    resize: Resize,
    target: RasterTarget,
    keep: MetadataCarry,
) -> Result<Vec<u8>, ExportError> {
    Ok(anyview_image::encode_file(file, resize, target, keep)?)
}

/// `page` of the PDF `file` drawn at `dpi` and encoded. A page has no metadata to carry.
pub(crate) fn pdf_page(
    session: &mut Session,
    file: &FilePath,
    page: PageIndex,
    dpi: Dpi,
    target: RasterTarget,
) -> Result<Vec<u8>, ExportError> {
    let doc = session.document(file)?;
    let job = PdfJob::Page {
        ticket: Ticket(0),
        page,
        dpi,
    };
    let PdfDone::Page { raster, .. } =
        PdfBackend::run(&doc, &mut session.worker, job, &Stop::new())
    else {
        return Err(ExportError::NothingToWrite);
    };
    let raster = raster?;
    let picture = Rgba8::new(raster.size(), raster.straight_rgba())?;
    Ok(anyview_image::encode(&picture, target)?)
}
