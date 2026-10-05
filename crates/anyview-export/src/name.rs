//! The name an export is written under: beside the original, never one that exists.

use anyview_core::{ExportExtension, ExportJob, PageSelection, PdfPages, PixelSource};
use ds_core::word::Word;
use std::path::{Path, PathBuf};

/// The most names tried before giving up: a folder with that many copies is not one to add to.
const MOST_TRIED: u32 = 10_000;

/// `<stem><suffix>.<extension>` beside `file`, or with ` 2`, ` 3`, ... before the extension: the
/// first that is free.
pub fn free_beside(file: &Path, suffix: &str, extension: &str) -> Option<PathBuf> {
    let folder = file.parent()?;
    let stem = file.file_stem()?.to_string_lossy().into_owned();
    (1..=MOST_TRIED)
        .map(|n| match n {
            1 => format!("{stem}{suffix}.{extension}"),
            n => format!("{stem}{suffix} {n}.{extension}"),
        })
        .map(|name| folder.join(name))
        .find(|candidate| !candidate.exists())
}

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
    fn a_name_beside_the_file_is_the_first_free_one() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Holiday.mkv");
        let name = |text: &str| dir.path().join(text);
        assert_eq!(
            free_beside(&file, " frame", "png"),
            Some(name("Holiday frame.png"))
        );
        std::fs::write(name("Holiday frame.png"), "x").unwrap();
        assert_eq!(
            free_beside(&file, " frame", "png"),
            Some(name("Holiday frame 2.png")),
            "a taken name is never overwritten"
        );
        std::fs::write(name("Holiday frame 2.png"), "x").unwrap();
        assert_eq!(
            free_beside(&file, " frame", "png"),
            Some(name("Holiday frame 3.png"))
        );
        assert_eq!(
            free_beside(&file, " frame", "jpg"),
            Some(name("Holiday frame.jpg")),
            "another extension is another name"
        );
    }

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
