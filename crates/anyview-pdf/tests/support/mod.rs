//! A three-page PDF built in memory: text on every page, an outline of four entries, two link
//! annotations and a filled rectangle. Small enough to read in full, and real enough to draw.
//!
//! Page 1 (612 x 792): "Chapter One", a sentence with "fox" once, a red box, a link to page 3 and
//! a link to a web address. Page 2 (612 x 792): "Chapter Two" and a sentence with "fox" and
//! "Foxes". Page 3 (400 x 300): "Appendix".

#![allow(dead_code, reason = "each test file uses part of the helpers")]

use anyview_pdf::{
    Backend, End, Hits, PdfBackend, PdfDocument, PdfDone, PdfError, PdfJob, PdfWorker, Raster,
    Stop, Ticket, Tile,
};

const CONTENT_ONE: &str = "BT /F1 36 Tf 72 700 Td (Chapter One) Tj ET\n\
BT /F1 14 Tf 72 650 Td (The quick brown fox jumps over the lazy dog.) Tj ET\n\
0.9 0.1 0.1 rg 72 400 200 100 re f\n";
const CONTENT_TWO: &str = "BT /F1 24 Tf 72 700 Td (Chapter Two) Tj ET\n\
BT /F1 14 Tf 72 650 Td (A second fox appears. Foxes are quick.) Tj ET\n";
const CONTENT_THREE: &str = "BT /F1 18 Tf 20 250 Td (Appendix) Tj ET\n";

/// The file's objects, numbered from 1.
fn objects() -> Vec<String> {
    let stream = |text: &str| format!("<< /Length {} >>\nstream\n{text}endstream", text.len());
    let page = |size: &str, contents: u32, extra: &str| {
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {size}] /Contents {contents} 0 R \
             /Resources << /Font << /F1 8 0 R >> >> {extra}>>"
        )
    };
    vec![
        "<< /Type /Catalog /Pages 2 0 R /Outlines 9 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>".into(),
        page("612 792", 6, "/Annots [12 0 R 13 0 R] "),
        page("612 792", 7, ""),
        page("400 300", 14, ""),
        stream(CONTENT_ONE),
        stream(CONTENT_TWO),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
        "<< /Type /Outlines /First 10 0 R /Last 16 0 R /Count 4 >>".into(),
        "<< /Title (Chapter One) /Parent 9 0 R /Next 11 0 R /First 15 0 R /Last 15 0 R /Count 1 \
         /Dest [3 0 R /Fit] >>"
            .into(),
        "<< /Title (Chapter Two) /Parent 9 0 R /Prev 10 0 R /Next 16 0 R /Dest [4 0 R /Fit] >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [72 650 300 670] /Border [0 0 0] /Dest [5 0 R /Fit] >>"
            .into(),
        "<< /Type /Annot /Subtype /Link /Rect [72 580 200 600] /Border [0 0 0] \
         /A << /S /URI /URI (https://example.com/) >> >>"
            .into(),
        stream(CONTENT_THREE),
        "<< /Title (Section 1.1) /Parent 10 0 R /Dest [3 0 R /Fit] >>".into(),
        "<< /Title (Appendix) /Parent 9 0 R /Prev 11 0 R /Dest [5 0 R /Fit] >>".into(),
    ]
}

/// The fixture's bytes, with a correct cross-reference table.
pub fn fixture_bytes() -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects().iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// The fixture, opened.
pub fn fixture() -> PdfDocument {
    PdfDocument::from_bytes(fixture_bytes()).expect("the fixture opens")
}

/// The pixel at (`x`, `y`) of a raster, `R G B A`.
pub fn pixel(raster: &Raster, x: u32, y: u32) -> [u8; 4] {
    let width = raster.size().width.0 as usize;
    let at = (y as usize * width + x as usize) * 4;
    let bytes = raster.premultiplied_rgba();
    [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
}

/// The rows `y..y + height` and columns `x..x + width` of a raster, as bytes.
pub fn crop(raster: &Raster, x: u32, y: u32, width: u32, height: u32) -> Vec<u8> {
    let stride = raster.size().width.0 as usize * 4;
    let mut out = Vec::new();
    for row in y..y + height {
        let from = row as usize * stride + x as usize * 4;
        out.extend_from_slice(&raster.premultiplied_rgba()[from..from + width as usize * 4]);
    }
    out
}

/// Whether any pixel in the box is darker than mid grey (text is black on white).
pub fn has_ink(raster: &Raster, x: u32, y: u32, width: u32, height: u32) -> bool {
    (y..y + height).any(|row| {
        (x..x + width).any(|column| {
            let [r, g, b, _] = pixel(raster, column, row);
            u32::from(r) + u32::from(g) + u32::from(b) < 3 * 128
        })
    })
}

/// Runs one job on a fresh worker with a stop that is never raised.
pub fn run(doc: &PdfDocument, job: PdfJob) -> PdfDone {
    PdfBackend::run(doc, &mut PdfWorker::new(), job, &Stop::new())
}

/// The tiles a finished tile job holds, and how it ended.
pub fn tiles(done: PdfDone) -> (Ticket, Vec<Tile>, End) {
    let PdfDone::Tiles { ticket, tiles, end } = done else {
        panic!("not tiles: {done:?}")
    };
    (ticket, tiles, end)
}

/// The page a finished page job drew.
pub fn page(done: PdfDone) -> (Ticket, Result<Raster, PdfError>) {
    let PdfDone::Page { ticket, raster } = done else {
        panic!("not a page: {done:?}")
    };
    (ticket, raster)
}

/// The hits a finished search holds, and how it ended.
pub fn searched(done: PdfDone) -> (Ticket, Hits, End) {
    let PdfDone::Searched { ticket, hits, end } = done else {
        panic!("not a search: {done:?}")
    };
    (ticket, hits, end)
}

/// The text a finished text job wrote.
pub fn text(done: PdfDone) -> Result<String, PdfError> {
    let PdfDone::Text { text, .. } = done else {
        panic!("not text: {done:?}")
    };
    text
}

/// The file a finished write or edit job wrote.
pub fn written(done: PdfDone) -> Result<Vec<u8>, PdfError> {
    let PdfDone::Written { file, .. } = done else {
        panic!("not a file: {done:?}")
    };
    file
}

/// A whole page drawn at `dpi` on a fresh worker.
pub fn drawn(doc: &PdfDocument, page_index: u32, dpi: anyview_core::Dpi) -> Raster {
    let job = PdfJob::Page {
        ticket: Ticket(1),
        page: anyview_core::PageIndex(page_index),
        dpi,
    };
    page(run(doc, job)).1.expect("the page draws")
}
