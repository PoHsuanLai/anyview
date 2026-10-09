//! PDF exports: a page range, pages as images, and the text of the document.

use crate::support;

use anyview_core::{
    Dpi, ExportChoice, FormatDetail, PageIndex, PageRange, PageSelection, PdfExport, RasterFormat,
    RasterTarget, TextExport,
};
use anyview_export::DocumentExport;
use anyview_pdf::{PagePicture, PdfDocument, pdf_of_pictures};
use support::{exported, flat, names, opened, size_of, write};

/// A PDF of three pages, each a flat colour of a different size.
fn pictures_pdf(dir: &std::path::Path) -> std::path::PathBuf {
    let page = |width, height| {
        let picture = flat(width, height, [10, 120, 200, 255]);
        PagePicture::Pixels {
            size: picture.size(),
            rgba: picture.into_bytes(),
        }
    };
    let bytes = pdf_of_pictures(&[page(40, 20), page(30, 30), page(20, 40)]).unwrap();
    write(dir, "pages.pdf", &bytes)
}

/// A PDF whose text is `markdown`, made the way a person would make one.
fn text_pdf(dir: &std::path::Path, markdown: &str) -> std::path::PathBuf {
    let source = write(dir, "notes.md", markdown.as_bytes());
    let [pdf] = exported(
        &source,
        DocumentExport::Text(TextExport::default_for(anyview_core::TextExportKind::Pdf)),
    )
    .try_into()
    .unwrap();
    pdf
}

fn range(first: u32, last: u32) -> PageSelection {
    PageSelection::Range(PageRange::new(PageIndex(first), PageIndex(last)).unwrap())
}

#[test]
fn a_page_range_is_a_pdf_of_those_pages_beside_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let source = pictures_pdf(dir.path());
    let before = std::fs::read(&source).unwrap();
    let [out] = exported(&source, DocumentExport::Pdf(PdfExport::Pdf(range(1, 2))))
        .try_into()
        .unwrap();
    assert_eq!(out.file_name().unwrap(), "pages pages 2-3.pdf");
    let doc = PdfDocument::open(&out).unwrap();
    assert_eq!(doc.page_count().get(), 2);
    assert_eq!(
        doc.page_size(PageIndex(0)).unwrap().width.0,
        22_500,
        "page 2 is 30 px square"
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
}

#[test]
fn pages_become_one_image_each_at_the_resolution_asked() {
    let dir = tempfile::tempdir().unwrap();
    let source = pictures_pdf(dir.path());
    let dpi = Dpi::new(144).unwrap();
    let outs = exported(
        &source,
        DocumentExport::Pdf(PdfExport::PageImages(range(0, 1), RasterTarget::Png, dpi)),
    );
    assert_eq!(
        names(dir.path()),
        ["pages page 1.png", "pages page 2.png", "pages.pdf"]
    );
    assert_eq!(outs.len(), 2);
    // 40 x 20 px is 30 x 15 points, which is 60 x 30 pixels at twice the page's own resolution.
    assert_eq!(size_of(&outs[0]), (60, 30));
    assert_eq!(size_of(&outs[1]), (45, 45));
    let (_, sniffed) = opened(&outs[0]);
    assert_eq!(sniffed.detail(), &FormatDetail::Raster(RasterFormat::Png));
}

#[test]
fn the_text_of_a_pdf_comes_out_plain_and_as_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = text_pdf(
        dir.path(),
        "# Quarterly review\n\nRevenue grew by a third.\n",
    );
    let [plain] = exported(&pdf, DocumentExport::Pdf(PdfExport::PlainText))
        .try_into()
        .unwrap();
    assert_eq!(plain.extension().unwrap(), "txt");
    let text = std::fs::read_to_string(plain).unwrap();
    assert!(
        text.contains("Quarterly review") && text.contains("Revenue grew"),
        "{text}"
    );
    let [markdown] = exported(&pdf, DocumentExport::Pdf(PdfExport::Markdown))
        .try_into()
        .unwrap();
    assert_eq!(markdown.extension().unwrap(), "md");
    let text = std::fs::read_to_string(markdown).unwrap();
    assert!(text.contains("Quarterly review"), "{text}");
}
