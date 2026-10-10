//! Files built to hurt: a header that claims a canvas of gigapixels, thousands of frames, a
//! drawing that asks for hours of filtering, a document nested a hundred thousand deep. Each is
//! small, and each must be answered with a refusal at once, in a peek and in the viewer, without
//! decoding what the header claims.

use crate::support;

use anyview_core::{Peek, PixelLen, PixelSize};
use anyview_image::{Decoded, FrameCount, ImageError, RasterPeek, VectorPeek, decode_bytes};
use std::time::{Duration, Instant};
use support::{budget, bytes, fixture, sniffed};

/// How long a refusal may take: it reads a header, so a second is generous even on a loaded
/// machine, and a decode of what the header claims takes far longer than this.
const REFUSAL: Duration = Duration::from_secs(5);

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

/// A GIF with a `canvas` square canvas and `frames` frames of one pixel each.
fn gif_of_tiny_frames(canvas: u16, frames: usize) -> Vec<u8> {
    let mut out = b"GIF89a".to_vec();
    out.extend(canvas.to_le_bytes());
    out.extend(canvas.to_le_bytes());
    // A global colour table of two entries, then the background and aspect bytes.
    out.extend([0x80, 0, 0]);
    out.extend([0, 0, 0, 255, 255, 255]);
    for _ in 0..frames {
        out.extend([0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
        out.extend([0x02, 0x02, 0x44, 0x01, 0x00]);
    }
    out.push(0x3B);
    out
}

fn refused<T>(got: &Result<T, ImageError>) -> bool {
    matches!(got, Err(ImageError::TooLarge { .. }))
}

fn refused_in_time(name: &str, started: Instant, got: &Result<impl std::fmt::Debug, ImageError>) {
    assert!(
        started.elapsed() < REFUSAL,
        "{name}: took {:?}",
        started.elapsed()
    );
    assert!(
        matches!(
            got,
            Err(ImageError::TooLarge { .. } | ImageError::Decode { .. })
        ),
        "{name}: {got:?}"
    );
}

#[test]
fn a_header_that_claims_a_huge_canvas_is_refused_before_anything_is_decoded() {
    // name, fixture
    const CASES: &[(&str, &str)] = &[
        ("a 65535 square gif of 72 bytes", "bomb-canvas.gif"),
        (
            "a 16000 square gif of three frames",
            "bomb-canvas-3-frames.gif",
        ),
        ("a 40000 square animated webp", "bomb-canvas.webp"),
        ("an animated webp 16.7 million wide", "bomb-frames.webp"),
        ("a photoshop header with no pixels", "bomb-canvas.psd"),
        ("an openexr of 84 megapixels in f32", "bomb-canvas.exr"),
        ("a radiance hdr of 16k square", "bomb-canvas.hdr"),
    ];
    for (name, file) in CASES {
        let (src, sniffed) = fixture(file);
        let started = Instant::now();
        let peeked = RasterPeek::peek(&src, &sniffed, &budget(1 << 20));
        refused_in_time(name, started, &peeked);
        assert!(
            matches!(peeked, Err(ImageError::TooLarge { .. })),
            "{name}: a peek is {peeked:?}"
        );
        let started = Instant::now();
        let opened = decode_bytes(&bytes(file), &sniffed);
        refused_in_time(name, started, &opened);
        assert!(
            matches!(opened, Err(ImageError::TooLarge { .. })),
            "{name}: a view is {opened:?}"
        );
    }
}

/// A 24-bit BMP header of `width` x `height` and no pixels after it.
fn bmp_header(width: u32, height: u32) -> Vec<u8> {
    let mut out = b"BM".to_vec();
    out.extend(54u32.to_le_bytes());
    out.extend([0; 4]);
    out.extend(54u32.to_le_bytes());
    out.extend(40u32.to_le_bytes());
    out.extend(width.to_le_bytes());
    out.extend(height.to_le_bytes());
    out.extend([1, 0, 24, 0]);
    out.extend([0; 24]);
    out
}

#[test]
fn what_a_still_costs_at_its_peak_decides_not_its_area_alone() {
    // name, width, height, a peek refuses, a view refuses
    const CASES: &[(&str, u32, u32, bool, bool)] = &[
        ("a small picture", 800, 600, false, false),
        ("81 megapixels of rgb", 9000, 9000, true, false),
        ("268 megapixels of rgb", 16384, 16384, true, true),
    ];
    for (name, width, height, peek_refuses, view_refuses) in CASES {
        let file = bmp_header(*width, *height);
        let sniffed = sniffed(&file, "wide.bmp");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.bmp");
        std::fs::write(&path, &file).unwrap();
        let source = support::source_of(&path, file.len());
        let peeked = RasterPeek::peek(&source, &sniffed, &budget(1 << 20));
        let opened = decode_bytes(&file, &sniffed);
        assert_eq!(refused(&peeked), *peek_refuses, "{name}: peek {peeked:?}");
        assert_eq!(refused(&opened), *view_refuses, "{name}: view {opened:?}");
    }
}

#[test]
fn thousands_of_frames_are_neither_decoded_to_be_counted_nor_kept() {
    // 3000 frames on a 512 square canvas: a megabyte each once composited, three gigabytes kept.
    let file = gif_of_tiny_frames(512, 3000);
    let sniffed = sniffed(&file, "many.gif");
    let started = Instant::now();
    let opened = decode_bytes(&file, &sniffed).unwrap();
    let Decoded::HeldStill { picture, frames } = opened else {
        panic!("three gigabytes of frames are held as a still");
    };
    assert_eq!((picture.size(), frames), (size(512, 512), FrameCount(3000)));
    assert!(started.elapsed() < REFUSAL, "took {:?}", started.elapsed());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.gif");
    std::fs::write(&path, &file).unwrap();
    let source = support::source_of(&path, file.len());
    let started = Instant::now();
    let peeked = RasterPeek::peek(&source, &sniffed, &budget(1 << 20)).unwrap();
    assert_eq!(peeked.frames, FrameCount(3000));
    assert_eq!(peeked.source_size, size(512, 512));
    assert!(started.elapsed() < REFUSAL, "took {:?}", started.elapsed());
}

#[test]
fn a_frame_that_breaks_the_webp_decoder_is_an_error_not_a_panic() {
    let file = bytes("damaged-frame.webp");
    let sniffed = sniffed(&file, "damaged-frame.webp");
    // Either answer is fine; a panic, which fails this test, is not.
    let _ = decode_bytes(&file, &sniffed);
    let (src, _) = fixture("damaged-frame.webp");
    let _ = RasterPeek::peek(&src, &sniffed, &budget(1 << 20));
}

#[test]
fn an_icon_element_that_claims_four_gigabytes_is_refused() {
    let (src, sniffed) = fixture("bomb-element.icns");
    let started = Instant::now();
    let peeked = RasterPeek::peek(&src, &sniffed, &budget(1 << 20));
    refused_in_time("a 16 byte icns", started, &peeked);
}

/// A raw file named `.dng`: a TIFF header and `repeats` JPEG start markers.
fn raw_of_start_markers(repeats: usize) -> Vec<u8> {
    let mut file = b"II*\0\x08\0\0\0".to_vec();
    file.extend([0xFF, 0xD8].repeat(repeats));
    file
}

#[test]
fn a_raw_file_of_start_markers_has_no_preview_and_is_read_once() {
    let file = raw_of_start_markers(400_000);
    let sniffed = sniffed(&file, "quad.dng");
    let started = Instant::now();
    let got = decode_bytes(&file, &sniffed);
    assert!(matches!(got, Err(ImageError::NoPreview)), "{got:?}");
    assert!(started.elapsed() < REFUSAL, "took {:?}", started.elapsed());
}

fn svg_peek(file: &str) -> Result<anyview_image::ImagePeek, ImageError> {
    let (src, sniffed) = fixture(file);
    VectorPeek::peek(&src, &sniffed, &budget(1 << 20))
}

#[test]
fn a_drawing_that_asks_for_hours_of_filtering_is_refused() {
    // name, fixture
    const CASES: &[(&str, &str)] = &[
        ("a thousand octaves of turbulence", "bomb-turbulence.svg"),
        ("a morphology of radius two thousand", "bomb-morphology.svg"),
        (
            "a blur over a region of twenty thousand pages",
            "bomb-blur-region.svg",
        ),
    ];
    for (name, file) in CASES {
        let started = Instant::now();
        let got = svg_peek(file);
        refused_in_time(name, started, &got);
        assert!(got.is_err(), "{name}");
    }
}

#[test]
fn a_drawing_nested_past_the_parsers_stack_is_refused_on_a_small_stack() {
    // The launcher's worker has 2 MiB: 6000 open elements overflowed it, 100 000 overflow 8 MiB.
    for depth in [6000, 100_000] {
        let text = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\">{}</svg>",
            "<g>".repeat(depth)
        );
        let worker = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                let sniffed = sniffed(text.as_bytes(), "deep.svg");
                decode_bytes(text.as_bytes(), &sniffed)
            })
            .unwrap();
        let got = worker
            .join()
            .expect("a deep document does not overflow the stack");
        assert!(
            matches!(got, Err(ImageError::Decode { .. })),
            "depth {depth}: {got:?}"
        );
    }
}
