//! Peeking at the fixtures: the downscaled picture, and the facts against recorded expectations.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FactLabel, FormatKind, Input, Peek, PixelLen, PixelSize};
use anyview_image::{FrameCount, ImageError, RasterPeek, VectorPeek};
use support::{budget, fixture};

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

/// The facts as `(label slug, text)` rows, in order.
fn rows<P: Peek>(peeked: &P::Peeked) -> Vec<(&'static str, String)> {
    use ds_core::word::Word;
    P::facts(peeked)
        .rows()
        .iter()
        .map(|fact| (fact.label.slug(), fact.value.as_str().to_owned()))
        .collect()
}

fn expected(rows: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    rows.iter().map(|(l, v)| (*l, (*v).to_owned())).collect()
}

#[test]
fn a_png_is_reduced_to_the_pixel_budget_with_its_proportions() {
    let (src, sniffed) = fixture("quadrants.png");
    let peeked = RasterPeek::peek(&Input::from(&src), &sniffed, &budget(100)).unwrap();
    assert_eq!(peeked.picture.size(), size(12, 8));
    assert_eq!(peeked.source_size, size(48, 32));
    assert_eq!(peeked.frames, FrameCount(1));
    assert_eq!(
        rows::<RasterPeek>(&peeked),
        expected(&[
            ("kind", "PNG image"),
            ("dimensions", "48 × 32"),
            ("colour", "RGBA, 8-bit"),
        ])
    );
}

#[test]
fn a_budget_larger_than_the_image_leaves_it_whole() {
    let (src, sniffed) = fixture("quadrants.png");
    let peeked = RasterPeek::peek(&Input::from(&src), &sniffed, &budget(1_000_000)).unwrap();
    assert_eq!(peeked.picture.size(), size(48, 32));
}

#[test]
fn the_byte_budget_limits_the_picture_as_well_as_the_pixel_budget() {
    let (src, sniffed) = fixture("quadrants.png");
    let mut tight = budget(1_000_000);
    tight.bytes = anyview_core::ByteLen(400);
    let peeked = RasterPeek::peek(&Input::from(&src), &sniffed, &tight).unwrap();
    assert_eq!(peeked.picture.size(), size(12, 8));
    assert!(peeked.picture.bytes().len() <= 400);
}

#[test]
fn an_exif_photo_peeks_upright_with_its_camera_facts() {
    let (src, sniffed) = fixture("rotated.jpg");
    let peeked = RasterPeek::peek(&Input::from(&src), &sniffed, &budget(1_000_000)).unwrap();
    assert_eq!(peeked.picture.size(), size(32, 48), "orientation applied");
    assert_eq!(
        rows::<RasterPeek>(&peeked),
        expected(&[
            ("kind", "JPEG image"),
            ("dimensions", "32 × 48"),
            ("colour", "RGB, 8-bit"),
            ("camera", "TestCam One"),
            ("lens", "TestLens 35mm f/2"),
            ("exposure", "1/200 s · f/2.8 · ISO 100 · 35 mm"),
            ("taken", "1 May 2024 at 12:30"),
        ])
    );
}

#[test]
fn an_animation_peeks_as_its_first_frame_and_a_frame_count() {
    let (src, sniffed) = fixture("spin.gif");
    let peeked = RasterPeek::peek(&Input::from(&src), &sniffed, &budget(1_000_000)).unwrap();
    assert_eq!(peeked.frames, FrameCount(3));
    assert_eq!(
        &peeked.picture.bytes()[..4],
        [255, 0, 0, 255],
        "first frame is red"
    );
    assert_eq!(
        rows::<RasterPeek>(&peeked),
        expected(&[
            ("kind", "GIF image"),
            ("dimensions", "32 × 24"),
            ("frames", "3"),
            ("colour", "RGBA, 8-bit"),
        ])
    );
}

#[test]
fn a_jpeg_xl_and_a_webp_report_their_formats() {
    for (name, kind) in [("photo.jxl", "JXL image"), ("lossy.webp", "WEBP image")] {
        let (src, sniffed) = fixture(name);
        let peeked = RasterPeek::peek(&Input::from(&src), &sniffed, &budget(1_000_000)).unwrap();
        let facts = RasterPeek::facts(&peeked);
        assert_eq!(
            facts.value(FactLabel::Kind).map(|v| v.as_str()),
            Some(kind),
            "{name}"
        );
        assert_eq!(peeked.source_size, size(48, 32), "{name}");
    }
}

#[test]
fn an_svg_is_drawn_inside_the_budget_and_reports_its_own_size() {
    let (src, sniffed) = fixture("logo.svg");
    let peeked = VectorPeek::peek(&Input::from(&src), &sniffed, &budget(200)).unwrap();
    assert_eq!(peeked.source_size, size(64, 32));
    assert_eq!(peeked.picture.size(), size(20, 10));
    assert_eq!(
        rows::<VectorPeek>(&peeked),
        expected(&[("kind", "SVG image"), ("dimensions", "64 × 32")])
    );
    assert_eq!(&peeked.picture.bytes()[..4], [255, 0, 0, 255], "red half");
}

#[test]
fn a_peek_refuses_another_kind_and_an_empty_budget() {
    let (svg, svg_sniffed) = fixture("logo.svg");
    assert_eq!(
        RasterPeek::peek(&Input::from(&svg), &svg_sniffed, &budget(100)),
        Err(ImageError::WrongKind {
            kind: FormatKind::Vector
        })
    );
    let (png, png_sniffed) = fixture("quadrants.png");
    assert_eq!(
        RasterPeek::peek(&Input::from(&png), &png_sniffed, &budget(0)),
        Err(ImageError::NoBudget)
    );
}
