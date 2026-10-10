//! A PDF export as the jobs it is made of, and the writers for the jobs that are not drawing: a
//! page range as a PDF, text, Markdown. Page images are drawn by a `PdfJob::Page` and encoded by
//! the caller (`anyview-image` owns every raster encoder).

use crate::document::PdfDocument;
use crate::edit::save;
use crate::error::PdfError;
use anyview_core::{
    ExportJob, FilePath, MetadataCarry, PageCount, PageIndex, PageSelection, PdfExport, PdfPages,
    PixelSource, TextFlavour, TextSource,
};
use pdfrum_edit::EditDoc;

/// The jobs that write `export` for the PDF `file` of `count` pages: one for a PDF or a text
/// file, one per page for page images (a range that starts past the end has none). The jobs are
/// independent, so a pool may run them in parallel. Page images carry no metadata: a page has
/// none to keep.
#[must_use]
pub fn plan_export(file: &FilePath, export: PdfExport, count: PageCount) -> Vec<ExportJob> {
    let text = |pages, flavour| ExportJob::WriteText {
        text: TextSource::Pdf {
            file: file.clone(),
            pages,
            flavour,
        },
    };
    match export {
        PdfExport::Pdf(pages) => vec![ExportJob::WritePdf {
            pages: PdfPages::Pages {
                file: file.clone(),
                pages,
            },
        }],
        PdfExport::PageImages(pages, target, dpi) => selected(pages, count)
            .into_iter()
            .map(|page| ExportJob::EncodeRaster {
                pixels: PixelSource::PdfPage {
                    file: file.clone(),
                    page,
                    dpi,
                },
                target,
                keep: MetadataCarry::Drop,
            })
            .collect(),
        PdfExport::PlainText => vec![text(PageSelection::All, TextFlavour::Plain)],
        PdfExport::Markdown => vec![text(PageSelection::All, TextFlavour::Markdown)],
    }
}

/// The pages a selection names in a document of `count` pages, cut to it.
pub fn selected(pages: PageSelection, count: PageCount) -> Vec<PageIndex> {
    match pages {
        PageSelection::All => (0..count.get()).map(PageIndex).collect(),
        PageSelection::Range(range) => range
            .within(count)
            .map(|range| (range.first().0..=range.last().0).map(PageIndex).collect())
            .unwrap_or_default(),
    }
}

/// The pages of `selection` as a PDF file (outline and links to pages left out are dropped with
/// the pages they lead to). All pages is the file as it is.
pub fn write_pages(doc: &PdfDocument, selection: PageSelection) -> Result<Vec<u8>, PdfError> {
    let keep = selected(selection, doc.page_count());
    if keep.is_empty() {
        return Err(PdfError::NoPages);
    }
    if keep.len() == doc.page_count().get() as usize {
        return Ok(doc.bytes().to_vec());
    }
    let doomed: Vec<u32> = (0..doc.page_count().get())
        .filter(|page| !keep.contains(&PageIndex(*page)))
        .collect();
    let mut edit = EditDoc::new(doc.inner().parser());
    pdfrum_edit::delete_pages(&mut edit, &pdfrum_edit::PageRange::of(doomed))?;
    save(&edit)
}

/// The text of the pages of `selection`, one block per page: plain text keeps its columns and
/// gaps, Markdown reads headings, lists and tables, and drops a header or footer repeated on most
/// pages. Pages are separated by a blank line (plain) or a rule (Markdown).
pub fn write_text(
    doc: &PdfDocument,
    selection: PageSelection,
    flavour: TextFlavour,
) -> Result<String, PdfError> {
    let pages = selected(selection, doc.page_count());
    if pages.is_empty() {
        return Err(PdfError::NoPages);
    }
    match flavour {
        TextFlavour::Plain => {
            let texts = pages
                .into_iter()
                .map(|page| doc.page(page).map(|sheet| sheet.layout_text()))
                .collect::<Result<Vec<String>, PdfError>>()?;
            Ok(texts.join("\n\n"))
        }
        TextFlavour::Markdown => {
            let blocks = doc
                .inner()
                .markdown_blocks(pages.into_iter().map(|page| page.0))?;
            let rendered: Vec<String> = blocks
                .iter()
                .map(|page| pdfrum::markdown::render(page))
                .collect();
            Ok(rendered.join("\n---\n\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{Dpi, PageRange, Percent, Quality, RasterTarget};

    fn count(pages: u32) -> PageCount {
        PageCount::new(pages).unwrap()
    }

    fn range(first: u32, last: u32) -> PageSelection {
        PageSelection::Range(PageRange::new(PageIndex(first), PageIndex(last)).unwrap())
    }

    #[test]
    fn a_selection_is_cut_to_the_document() {
        type Row = (&'static str, Option<(u32, u32)>, u32, &'static [u32]);
        const CASES: &[Row] = &[
            // name, range (None is all), pages, selected
            ("all", None, 3, &[0, 1, 2]),
            ("a run", Some((1, 2)), 5, &[1, 2]),
            ("a run past the end", Some((2, 9)), 4, &[2, 3]),
            ("starts past the end", Some((7, 9)), 4, &[]),
        ];
        for (name, span, pages, want) in CASES {
            let selection = match span {
                Some((first, last)) => range(*first, *last),
                None => PageSelection::All,
            };
            let got: Vec<u32> = selected(selection, count(*pages))
                .iter()
                .map(|p| p.0)
                .collect();
            assert_eq!(&got, want, "{name}");
        }
    }

    fn jobs(export: PdfExport, pages: u32) -> Vec<ExportJob> {
        let file = FilePath::new("/docs/book.pdf").unwrap();
        plan_export(&file, export, count(pages))
    }

    fn page_image(page: u32, dpi: Dpi, target: RasterTarget) -> ExportJob {
        ExportJob::EncodeRaster {
            pixels: PixelSource::PdfPage {
                file: FilePath::new("/docs/book.pdf").unwrap(),
                page: PageIndex(page),
                dpi,
            },
            target,
            keep: MetadataCarry::Drop,
        }
    }

    fn text_job(pages: PageSelection, flavour: TextFlavour) -> ExportJob {
        ExportJob::WriteText {
            text: TextSource::Pdf {
                file: FilePath::new("/docs/book.pdf").unwrap(),
                pages,
                flavour,
            },
        }
    }

    #[test]
    fn each_export_is_planned_as_its_jobs() {
        let jpeg = RasterTarget::Jpeg(Quality::clamped(Percent(80)));
        assert_eq!(
            jobs(PdfExport::PageImages(range(1, 2), jpeg, Dpi::PRINT), 5),
            [
                page_image(1, Dpi::PRINT, jpeg),
                page_image(2, Dpi::PRINT, jpeg)
            ]
        );
        assert_eq!(
            jobs(PdfExport::Pdf(range(0, 0)), 5),
            [ExportJob::WritePdf {
                pages: PdfPages::Pages {
                    file: FilePath::new("/docs/book.pdf").unwrap(),
                    pages: range(0, 0),
                }
            }]
        );
        assert_eq!(
            jobs(PdfExport::PlainText, 5),
            [text_job(PageSelection::All, TextFlavour::Plain)]
        );
        assert_eq!(
            jobs(PdfExport::Markdown, 5),
            [text_job(PageSelection::All, TextFlavour::Markdown)]
        );
        assert!(
            jobs(
                PdfExport::PageImages(range(8, 9), RasterTarget::Png, Dpi::SCREEN),
                5
            )
            .is_empty()
        );
    }
}
