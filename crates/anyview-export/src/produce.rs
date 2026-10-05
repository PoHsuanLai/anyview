//! A job as the bytes of the file it writes: the one match on `ExportJob` in this crate.

use crate::error::ExportError;
use crate::session::Session;
use crate::{pdf, print, raster};
use anyview_core::{ExportJob, PdfPages, PixelSource, TextSource};

/// The file `job` writes, whole. Blocking, and slow for AVIF at a large size or a long document.
pub(crate) fn produce(session: &mut Session, job: &ExportJob) -> Result<Vec<u8>, ExportError> {
    match job {
        ExportJob::EncodeRaster {
            pixels,
            target,
            keep,
        } => match pixels {
            PixelSource::Image { file, resize } => raster::image(file, *resize, *target, *keep),
            PixelSource::PdfPage { file, page, dpi } => {
                raster::pdf_page(session, file, *page, *dpi, *target)
            }
        },
        ExportJob::WritePdf { pages } => match pages {
            PdfPages::Pages { file, pages } => pdf::pages(session, file, *pages),
            PdfPages::Images(files) => pdf::of_images(files),
        },
        ExportJob::PrintToPdf { html, layout } => print::print_html(session, html, *layout),
        ExportJob::WriteText { text } => match text {
            TextSource::Pdf {
                file,
                pages,
                flavour,
            } => pdf::text(session, file, *pages, *flavour),
            TextSource::File(file) => {
                std::fs::read(file.as_path()).map_err(|error| ExportError::Read {
                    path: file.as_path().to_path_buf(),
                    kind: error.kind(),
                })
            }
        },
        ExportJob::MpvScreenshot { .. } | ExportJob::Transcode { .. } => {
            Err(ExportError::NotADocument)
        }
    }
}
