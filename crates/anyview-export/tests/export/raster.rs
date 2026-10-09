//! Image exports: each target decodes back as what was asked, resized, with metadata carried
//! when it is to be, and an image placed on a PDF page.

use crate::support;

use anyview_core::{
    FormatDetail, MetadataCarry, PixelLen, RasterExport, RasterFormat, RasterTarget, Resize,
};
use anyview_export::DocumentExport;
use anyview_image::ExifFacts;
use anyview_pdf::PdfDocument;
use support::{exported, fixture, flat, names, opened, size_of, write};

fn image(target: RasterTarget, resize: Resize) -> DocumentExport {
    DocumentExport::Raster(RasterExport::Image(target, resize, MetadataCarry::Keep))
}

fn png_in(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let picture = flat(8, 6, [200, 30, 30, 255]);
    write(
        dir,
        name,
        &anyview_image::encode(&picture, RasterTarget::Png).unwrap(),
    )
}

#[test]
fn every_target_writes_a_file_that_is_that_format_at_the_size_of_the_picture() {
    const CASES: &[(&str, RasterTarget, RasterFormat)] = &[
        ("png", RasterTarget::Png, RasterFormat::Png),
        ("webp", RasterTarget::Webp, RasterFormat::Webp),
        ("tiff", RasterTarget::Tiff, RasterFormat::Tiff),
    ];
    let dir = tempfile::tempdir().unwrap();
    let source = png_in(dir.path(), "red.png");
    for (name, target, format) in CASES {
        let [out] = exported(&source, image(*target, Resize::Original))
            .try_into()
            .unwrap();
        assert_eq!(size_of(&out), (8, 6), "{name}");
        let (_, sniffed) = opened(&out);
        assert_eq!(
            sniffed.detail(),
            &FormatDetail::Raster(*format),
            "{name} is written as its own format"
        );
    }
    let [jpeg] = exported(
        &source,
        image(RasterTarget::default_jpeg(), Resize::Original),
    )
    .try_into()
    .unwrap();
    assert_eq!(size_of(&jpeg), (8, 6), "jpeg");
    assert_eq!(jpeg.extension().unwrap(), "jpg");
    let [avif] = exported(
        &source,
        image(RasterTarget::default_avif(), Resize::Original),
    )
    .try_into()
    .unwrap();
    let (_, sniffed) = opened(&avif);
    assert_eq!(
        sniffed.detail(),
        &FormatDetail::Raster(RasterFormat::Avif),
        "avif"
    );
}

#[test]
fn a_resize_changes_the_size_of_the_file_written() {
    let dir = tempfile::tempdir().unwrap();
    let source = png_in(dir.path(), "red.png");
    let [out] = exported(
        &source,
        image(RasterTarget::Png, Resize::LongEdge(PixelLen(4))),
    )
    .try_into()
    .unwrap();
    assert_eq!(size_of(&out), (4, 3));
}

#[test]
fn an_export_is_a_copy_beside_the_original_which_is_never_touched() {
    let dir = tempfile::tempdir().unwrap();
    let source = png_in(dir.path(), "red.png");
    let before = std::fs::read(&source).unwrap();
    let [out] = exported(&source, image(RasterTarget::Png, Resize::Original))
        .try_into()
        .unwrap();
    assert_eq!(out.file_name().unwrap(), "red 2.png", "the free name");
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert_eq!(names(dir.path()), ["red 2.png", "red.png"], "no stray file");
}

#[test]
fn a_jpeg_keeps_its_exif_upright_when_the_export_keeps_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(dir.path(), "turned.jpg", &fixture("rotated.jpg"));
    let [out] = exported(
        &source,
        image(RasterTarget::default_jpeg(), Resize::Original),
    )
    .try_into()
    .unwrap();
    let facts = ExifFacts::read(&std::fs::read(&out).unwrap());
    assert_eq!(facts.camera().as_deref(), Some("TestCam One"));
    assert_eq!(facts.orientation.tag(), 1, "the pixels are already upright");
    assert_eq!(
        size_of(&out),
        size_of(&source),
        "and not turned a second time"
    );
}

#[test]
fn an_svg_is_drawn_as_a_picture_and_resized() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(dir.path(), "logo.svg", &fixture("logo.svg"));
    let [full] = exported(&source, image(RasterTarget::Png, Resize::Original))
        .try_into()
        .unwrap();
    assert_eq!(size_of(&full), (1024, 512), "drawn at the viewer's size");
    let [small] = exported(
        &source,
        image(RasterTarget::Png, Resize::LongEdge(PixelLen(64))),
    )
    .try_into()
    .unwrap();
    assert_eq!(size_of(&small), (64, 32));
}

#[test]
fn a_raster_image_becomes_one_pdf_page() {
    let dir = tempfile::tempdir().unwrap();
    let source = png_in(dir.path(), "red.png");
    let [out] = exported(&source, DocumentExport::Raster(RasterExport::Pdf))
        .try_into()
        .unwrap();
    assert_eq!(out.extension().unwrap(), "pdf");
    let doc = PdfDocument::open(&out).unwrap();
    assert_eq!(doc.page_count().get(), 1);
}

#[test]
fn an_upright_jpeg_is_stored_in_the_pdf_as_it_is_and_a_turned_one_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let plain = write(dir.path(), "plain.jpg", &fixture("plain.jpg"));
    let turned = write(dir.path(), "turned.jpg", &fixture("rotated.jpg"));
    let stored_as_is = |path: &std::path::Path| {
        let [out] = exported(path, DocumentExport::Raster(RasterExport::Pdf))
            .try_into()
            .unwrap();
        let pdf = std::fs::read(out).unwrap();
        let jpeg = std::fs::read(path).unwrap();
        pdf.windows(jpeg.len()).any(|window| window == jpeg)
    };
    assert!(stored_as_is(&plain));
    assert!(!stored_as_is(&turned));
}

#[test]
fn an_svg_becomes_a_vector_pdf_page() {
    let dir = tempfile::tempdir().unwrap();
    let source = write(dir.path(), "logo.svg", &fixture("logo.svg"));
    let [out] = exported(&source, DocumentExport::Raster(RasterExport::Pdf))
        .try_into()
        .unwrap();
    let doc = PdfDocument::open(&out).unwrap();
    assert_eq!(doc.page_count().get(), 1);
    let pdf = std::fs::read(out).unwrap();
    let has_image = pdf.windows(6).any(|window| window == b"/Image");
    assert!(!has_image, "the drawing is paths, not a bitmap");
}
