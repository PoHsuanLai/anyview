//! Exports: page ranges as PDFs, text, Markdown and page images, through the planned pieces.

mod support;

use anyview_core::{
    Dpi, PageIndex, PageRange, PageSelection, PdfExport, RasterTarget, TextFlavour,
};
use anyview_pdf::{
    ExportPiece, PdfDocument, PdfJob, PdfWorker, SearchQuery, Stop, Ticket, plan_export,
    search_document, write_pages, write_text,
};
use support::{drawn, fixture, page, run, text, written};

fn range(first: u32, last: u32) -> PageSelection {
    PageSelection::Range(PageRange::new(PageIndex(first), PageIndex(last)).expect("a range"))
}

#[test]
fn a_page_range_is_a_pdf_of_those_pages() {
    let doc = fixture();
    let bytes = write_pages(&doc, range(1, 2)).expect("written");
    let tail = PdfDocument::from_bytes(bytes).expect("opens");
    assert_eq!(tail.page_count().get(), 2);
    let query = SearchQuery::new("Chapter");
    let (hits, _) = search_document(&tail, &mut PdfWorker::new(), &query, &Stop::new());
    assert_eq!(hits.len(), 1);
    // All pages is the file as it is, and a range past the end is an error, not a blank file.
    assert_eq!(
        write_pages(&doc, PageSelection::All).expect("written"),
        doc.bytes()
    );
    assert!(write_pages(&doc, range(5, 6)).is_err());
}

#[test]
fn text_comes_out_plain_or_as_markdown() {
    let doc = fixture();
    let plain = write_text(&doc, PageSelection::All, TextFlavour::Plain).expect("text");
    assert!(plain.contains("Chapter One") && plain.contains("quick brown fox"));
    assert!(plain.contains("Appendix"));
    let second = write_text(&doc, range(1, 1), TextFlavour::Plain).expect("text");
    assert!(second.contains("Chapter Two") && !second.contains("Chapter One"));
    let markdown = write_text(&doc, PageSelection::All, TextFlavour::Markdown).expect("text");
    assert!(markdown.contains("Chapter One") && markdown.contains("\n---\n"));
}

#[test]
fn every_planned_piece_runs_as_a_job() {
    let doc = fixture();
    let dpi = Dpi::new(144).expect("a resolution");
    let images = PdfExport::PageImages(PageSelection::All, RasterTarget::Png, dpi);
    let plan = plan_export(images, doc.page_count());
    assert_eq!(plan.len(), 3);
    let ExportPiece::DrawPage {
        page: last, dpi, ..
    } = plan[2]
    else {
        panic!("a page: {:?}", plan[2])
    };
    let (_, raster) = page(run(
        &doc,
        PdfJob::Page {
            ticket: Ticket(1),
            page: last,
            dpi,
        },
    ));
    let size = raster.expect("drawn").size();
    assert_eq!((size.width.0, size.height.0), (800, 600));

    let [ExportPiece::WriteText { pages, flavour }] =
        plan_export(PdfExport::Markdown, doc.page_count())[..]
    else {
        panic!("one text piece")
    };
    let job = PdfJob::Text {
        ticket: Ticket(2),
        pages,
        flavour,
    };
    assert!(text(run(&doc, job)).expect("text").contains("Chapter Two"));

    let [ExportPiece::WritePdf(pages)] =
        plan_export(PdfExport::Pdf(range(0, 0)), doc.page_count())[..]
    else {
        panic!("one pdf piece")
    };
    let file = written(run(
        &doc,
        PdfJob::WritePdf {
            ticket: Ticket(3),
            pages,
        },
    ))
    .expect("file");
    assert_eq!(
        PdfDocument::from_bytes(file)
            .expect("opens")
            .page_count()
            .get(),
        1
    );
}

#[test]
fn page_pixels_are_opaque_so_encoders_may_take_either_alpha() {
    let raster = drawn(&fixture(), 0, Dpi::PAGE);
    assert_eq!(raster.straight_rgba(), raster.premultiplied_rgba());
}
