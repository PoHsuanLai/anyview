//! The name an export is written under: beside the original, never one that exists.

use anyview_core::{ExportExtension, ExportJob, PageSelection, PdfPages, PixelSource};
use anyview_store::free_beside;
use ds_core::word::Word;
use std::path::{Path, PathBuf};

/// What `job` adds to the original's name so the files of one export tell each other apart: the
/// page a picture is of, or the pages a PDF keeps.
fn suffix(job: &ExportJob) -> String {
    match job {
        ExportJob::EncodeRaster {
            pixels: PixelSource::PdfPage { page, .. },
            ..
        } => format!(" page {}", page.0 + 1),
        ExportJob::WritePdf {
            pages:
                PdfPages::Pages {
                    pages: PageSelection::Range(range),
                    ..
                },
        } => format!(" pages {}-{}", range.first().0 + 1, range.last().0 + 1),
        ExportJob::WritePdf {
            pages:
                PdfPages::Pages {
                    pages: PageSelection::All,
                    ..
                },
        } => " copy".to_owned(),
        ExportJob::EncodeRaster { .. }
        | ExportJob::WritePdf {
            pages: PdfPages::Images(_),
        }
        | ExportJob::PrintToPdf { .. }
        | ExportJob::WriteText { .. }
        | ExportJob::MpvScreenshot { .. }
        | ExportJob::Transcode { .. } => String::new(),
    }
}

/// Where the file `job` writes goes: beside `source`, with the extension the export has.
pub(crate) fn output_path(
    source: &Path,
    job: &ExportJob,
    extension: ExportExtension,
) -> Option<PathBuf> {
    free_beside(source, &suffix(job), extension.slug())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{Dpi, FilePath, MetadataCarry, PageIndex, PageRange, RasterTarget};

    #[test]
    fn each_job_names_what_tells_its_file_apart() {
        let file = FilePath::new("/docs/book.pdf").unwrap();
        let page = ExportJob::EncodeRaster {
            pixels: PixelSource::PdfPage {
                file: file.clone(),
                page: PageIndex(2),
                dpi: Dpi::SCREEN,
            },
            target: RasterTarget::Png,
            keep: MetadataCarry::Drop,
        };
        let pages = |selection| ExportJob::WritePdf {
            pages: PdfPages::Pages {
                file: file.clone(),
                pages: selection,
            },
        };
        let range = PageRange::new(PageIndex(1), PageIndex(3)).unwrap();
        let cases = [
            ("a page image", page, " page 3"),
            (
                "a page range",
                pages(PageSelection::Range(range)),
                " pages 2-4",
            ),
            ("every page", pages(PageSelection::All), " copy"),
        ];
        for (name, job, want) in cases {
            assert_eq!(suffix(&job), want, "{name}");
        }
    }
}
