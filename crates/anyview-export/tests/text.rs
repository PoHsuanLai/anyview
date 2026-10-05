//! Text exports: Markdown, source code and plain text as a PDF with real text, and as text.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{ExportChoice, PrintLayout, RasterTarget, TextExport, TextExportKind};
use anyview_export::DocumentExport;
use anyview_pdf::PdfDocument;
use support::{exported, flat, write};

fn pdf() -> DocumentExport {
    DocumentExport::Text(TextExport::default_for(TextExportKind::Pdf))
}

/// The text of the PDF at `path`.
fn text_of(path: &std::path::Path) -> String {
    let doc = PdfDocument::open(path).unwrap();
    anyview_pdf::write_text(
        &doc,
        anyview_core::PageSelection::All,
        anyview_core::TextFlavour::Plain,
    )
    .unwrap()
}

#[test]
fn markdown_becomes_a_pdf_whose_text_is_the_documents() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(
        dir.path(),
        "notes.md",
        b"# Field notes\n\nThe heron stood *very* still.\n\n- first item\n- second item\n",
    );
    let [out] = exported(&source, pdf()).try_into().unwrap();
    assert_eq!(out.file_name().unwrap(), "notes.pdf");
    let text = text_of(&out);
    for wanted in ["Field notes", "heron stood", "first item", "second item"] {
        assert!(text.contains(wanted), "{wanted:?} in {text:?}");
    }
    assert!(
        !text.contains('#') && !text.contains('*'),
        "rendered, not the source: {text:?}"
    );
}

#[test]
fn a_local_markdown_image_is_inlined_since_the_printer_loads_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let picture = flat(16, 16, [20, 160, 60, 255]);
    write(
        dir.path(),
        "dot.png",
        &anyview_image::encode(&picture, RasterTarget::Png).unwrap(),
    );
    let with = write(dir.path(), "with.md", b"Look:\n\n![a dot](dot.png)\n");
    let without = write(
        dir.path(),
        "without.md",
        b"Look:\n\n![a dot](missing.png)\n",
    );
    let has_image = |source: &std::path::Path| {
        let [out] = exported(source, pdf()).try_into().unwrap();
        std::fs::read(out)
            .unwrap()
            .windows(6)
            .any(|window| window == b"/Image")
    };
    assert!(has_image(&with), "the image is on the page");
    assert!(
        !has_image(&without),
        "an image that is not there is its alt text"
    );
}

#[test]
fn source_code_is_printed_with_its_text_whole() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(
        dir.path(),
        "main.rs",
        b"fn main() {\n    println!(\"hello, world\");\n}\n",
    );
    let [out] = exported(&source, pdf()).try_into().unwrap();
    let text = text_of(&out);
    assert!(
        text.contains("fn main()") && text.contains("hello, world"),
        "{text:?}"
    );
}

#[test]
fn an_html_file_is_printed_as_the_source_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(dir.path(), "page.html", b"<h1>Title</h1>\n<p>body</p>\n");
    let [out] = exported(&source, pdf()).try_into().unwrap();
    let text = text_of(&out);
    assert!(text.contains("<h1>Title</h1>"), "{text:?}");
}

#[test]
fn text_in_utf16_is_printed_as_text_not_as_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend("café crème\n".encode_utf16().flat_map(u16::to_le_bytes));
    let source = write(dir.path(), "menu.txt", &bytes);
    let [out] = exported(&source, pdf()).try_into().unwrap();
    assert!(text_of(&out).contains("café crème"));
}

#[test]
fn a_longer_document_runs_over_several_pages_and_landscape_is_wider() {
    let dir = tempfile::tempdir().unwrap();
    let lines: String = (0..400).map(|n| format!("line number {n}\n")).collect();
    let source = write(dir.path(), "long.txt", lines.as_bytes());
    let [portrait] = exported(&source, pdf()).try_into().unwrap();
    let doc = PdfDocument::open(&portrait).unwrap();
    assert!(
        doc.page_count().get() > 1,
        "{} pages",
        doc.page_count().get()
    );
    let landscape = DocumentExport::Text(TextExport::Pdf(PrintLayout {
        orientation: anyview_core::Orientation::Landscape,
        ..PrintLayout::default()
    }));
    let [wide] = exported(&source, landscape).try_into().unwrap();
    let wide = PdfDocument::open(&wide).unwrap();
    let size = wide.page_size(anyview_core::PageIndex(0)).unwrap();
    assert!(size.width.0 > size.height.0);
}

#[test]
fn plain_text_export_is_the_file_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let body = "# Not rendered\n\nJust the words.\n";
    let source = write(dir.path(), "notes.md", body.as_bytes());
    let [out] = exported(&source, DocumentExport::Text(TextExport::PlainText))
        .try_into()
        .unwrap();
    assert_eq!(out.file_name().unwrap(), "notes.txt");
    assert_eq!(std::fs::read_to_string(out).unwrap(), body);
}
