//! What an export never does: leave part of a file, touch the original or replace a file, and
//! what a printout is.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{
    ExportChoice, MetadataCarry, PageSelection, PdfExport, RasterExport, RasterTarget, Resize,
    TextExport, TextExportKind,
};
use anyview_export::{DocumentExport, ExportError, export, printout};
use support::{fixture, flat, names, opened, write};

#[test]
fn a_file_that_cannot_be_decoded_leaves_nothing_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = anyview_image::encode(&flat(8, 8, [1, 2, 3, 255]), RasterTarget::Png).unwrap();
    bytes.truncate(bytes.len() / 2);
    let source = write(dir.path(), "cut.png", &bytes);
    let (file, sniffed) = opened(&source);
    let error = export(
        &file,
        &sniffed,
        DocumentExport::Raster(RasterExport::Image(
            RasterTarget::Tiff,
            Resize::Original,
            MetadataCarry::Keep,
        )),
    )
    .unwrap_err();
    assert!(matches!(error, ExportError::Image(_)), "{error}");
    assert_eq!(
        names(dir.path()),
        ["cut.png"],
        "no output and no temporary file"
    );
}

#[test]
fn a_pdf_export_of_a_file_that_is_not_a_pdf_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(dir.path(), "notes.md", b"words");
    let (file, sniffed) = opened(&source);
    let error = export(
        &file,
        &sniffed,
        DocumentExport::Pdf(PdfExport::Pdf(PageSelection::All)),
    )
    .unwrap_err();
    assert!(matches!(error, ExportError::Pdf(_)), "{error}");
    assert_eq!(names(dir.path()), ["notes.md"]);
}

#[test]
fn a_choice_that_is_not_for_the_file_has_nothing_to_write() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(dir.path(), "photo.jpg", &fixture("plain.jpg"));
    let (file, sniffed) = opened(&source);
    let error = export(
        &file,
        &sniffed,
        DocumentExport::Text(TextExport::default_for(TextExportKind::Pdf)),
    )
    .unwrap_err();
    assert!(matches!(error, ExportError::NothingToWrite), "{error}");
    assert_eq!(names(dir.path()), ["photo.jpg"]);
}

#[test]
fn a_printout_is_a_pdf_for_an_image_a_document_and_a_pdf_and_nothing_for_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let image = write(dir.path(), "photo.jpg", &fixture("plain.jpg"));
    let notes = write(dir.path(), "notes.md", b"# Title\n\nBody.\n");
    let svg = write(dir.path(), "logo.svg", &fixture("logo.svg"));
    let pdf = write(dir.path(), "again.pdf", &printout_of(&notes));
    for (name, path) in [
        ("image", &image),
        ("markdown", &notes),
        ("svg", &svg),
        ("pdf", &pdf),
    ] {
        let (file, sniffed) = opened(path);
        let bytes = printout(&file, &sniffed).unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "{name}");
    }
    assert_eq!(names(dir.path()).len(), 4, "a printout writes no file");
    let audio = write(dir.path(), "tone.mp3", b"ID3\x04\0\0\0\0\0\0");
    let (file, sniffed) = opened(&audio);
    assert!(matches!(
        printout(&file, &sniffed),
        Err(ExportError::NothingToWrite)
    ));
}

fn printout_of(path: &std::path::Path) -> Vec<u8> {
    let (file, sniffed) = opened(path);
    printout(&file, &sniffed).unwrap()
}
