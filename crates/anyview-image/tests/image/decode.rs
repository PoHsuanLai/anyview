//! Decoding the fixtures: the pixels that come out, the orientation applied, animation timing.

use crate::support;

use anyview_core::{MediaTime, PixelLen, PixelSize};
use anyview_image::{Decoded, ImageError, Rgba8, declared_size, decode, decode_bytes};
use support::{bytes, fixture, sniffed};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

/// Whether every channel is within what a lossy encoder moves it.
fn near(got: [u8; 4], want: [u8; 4]) -> bool {
    got.iter().zip(want).all(|(g, w)| g.abs_diff(w) < 12)
}

fn still(name: &str) -> Rgba8 {
    let (src, sniffed) = fixture(name);
    match decode(&src, &sniffed).unwrap() {
        Decoded::Still(picture) => picture,
        Decoded::Animated(_) | Decoded::HeldStill { .. } => panic!("{name} is not a still"),
    }
}

/// The pixel at column `x`, row `y`.
fn at(picture: &Rgba8, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * picture.size().width.0 + x) * 4) as usize;
    picture.bytes()[i..i + 4].try_into().unwrap()
}

/// The pixel at each of the four corners, clockwise from the top left.
fn corners(picture: &Rgba8) -> [[u8; 4]; 4] {
    let (w, h) = (picture.size().width.0 - 1, picture.size().height.0 - 1);
    [
        at(picture, 0, 0),
        at(picture, w, 0),
        at(picture, w, h),
        at(picture, 0, h),
    ]
}

#[test]
fn a_png_keeps_its_quadrants_and_its_alpha() {
    let picture = still("quadrants.png");
    assert_eq!(picture.size(), size(48, 32));
    // The corner pixel was drawn half transparent when the fixture was made.
    assert_eq!(at(&picture, 0, 0), [255, 0, 0, 128]);
    assert_eq!(
        corners(&picture)[1..],
        [GREEN, YELLOW, BLUE],
        "other corners"
    );
}

#[test]
fn a_jpeg_decodes_to_close_to_its_colours() {
    let picture = still("plain.jpg");
    assert_eq!(picture.size(), size(48, 32));
    let [tl, tr, br, bl] = corners(&picture);
    for (name, got, want) in [
        ("tl", tl, RED),
        ("tr", tr, GREEN),
        ("br", br, YELLOW),
        ("bl", bl, BLUE),
    ] {
        assert!(near(got, want), "{name}: {got:?} is not near {want:?}");
    }
}

#[test]
fn exif_orientation_six_comes_back_upright() {
    let stored = still("plain.jpg");
    let rotated = still("rotated.jpg");
    // Stored 48 wide by 32 high; orientation 6 displays it turned a quarter clockwise.
    assert_eq!(stored.size(), size(48, 32));
    assert_eq!(rotated.size(), size(32, 48));
    let [tl, tr, br, bl] = corners(&rotated);
    // A quarter turn clockwise moves the stored bottom left to the top left.
    for (name, got, want) in [
        ("tl", tl, BLUE),
        ("tr", tr, RED),
        ("br", br, GREEN),
        ("bl", bl, YELLOW),
    ] {
        assert!(near(got, want), "{name}: {got:?} is not near {want:?}");
    }
}

#[test]
fn a_gif_has_a_frame_per_picture_and_the_delays_it_asked_for() {
    let (src, sniffed) = fixture("spin.gif");
    let Decoded::Animated(animation) = decode(&src, &sniffed).unwrap() else {
        panic!("three frames are an animation");
    };
    let delays: Vec<MediaTime> = animation.frames.iter().map(|f| f.delay).collect();
    assert_eq!(delays, [50, 200, 400].map(MediaTime::from_millis),);
    let colours: Vec<[u8; 4]> = animation
        .frames
        .iter()
        .map(|f| at(&f.pixels, 0, 0))
        .collect();
    assert_eq!(colours, [RED, GREEN, BLUE]);
    assert!(
        animation
            .frames
            .iter()
            .all(|f| f.pixels.size() == size(32, 24))
    );
}

#[test]
fn an_animated_webp_decodes_to_its_frames() {
    let (src, sniffed) = fixture("anim.webp");
    let Decoded::Animated(animation) = decode(&src, &sniffed).unwrap() else {
        panic!("three frames are an animation");
    };
    assert_eq!(animation.frames.count().get(), 3);
    // The encoder rounds a channel by one step.
    for (frame, want) in animation.frames.iter().zip([RED, GREEN, BLUE]) {
        assert!(near(at(&frame.pixels, 5, 5), want), "{want:?}");
    }
}

#[test]
fn a_still_webp_and_a_jpeg_xl_file_decode_as_stills() {
    // row, file, the corners it was made with (none: only the size is asserted)
    let cases = [
        ("a still webp", "lossy.webp", None),
        (
            "a jpeg xl file",
            "photo.jxl",
            Some([[255, 0, 0, 128], GREEN, YELLOW, BLUE]),
        ),
    ];
    for (row, file, want) in cases {
        let picture = still(file);
        assert_eq!(picture.size(), size(48, 32), "row {row}");
        if let Some(want) = want {
            assert_eq!(corners(&picture), want, "row {row}");
        }
    }
}

#[test]
fn an_svg_is_drawn_at_a_sharp_size_with_transparent_corners_left_clear() {
    let picture = still("logo.svg");
    // 64 by 32 is enlarged to a 1024 long edge.
    assert_eq!(picture.size(), size(1024, 512));
    assert_eq!(at(&picture, 10, 10), RED);
    assert_eq!(at(&picture, 1000, 500), BLUE);
}

#[test]
fn a_file_that_is_not_an_image_is_refused_by_kind() {
    let text = b"just words";
    let result = decode_bytes(text, &sniffed(text, "notes.txt"));
    assert_eq!(
        result,
        Err(ImageError::WrongKind {
            kind: anyview_core::FormatKind::PlainText
        })
    );
}

#[test]
fn a_truncated_image_is_a_decode_error_not_a_panic() {
    let png = bytes("quadrants.png");
    let cut = &png[..png.len() / 2];
    let result = decode_bytes(cut, &sniffed(&png, "a.png"));
    assert!(
        matches!(result, Err(ImageError::Decode { .. })),
        "{result:?}"
    );
}

#[test]
fn a_missing_file_names_its_path() {
    let (src, sniffed) = fixture("plain.jpg");
    let missing = anyview_core::Source::new(
        anyview_core::FilePath::new("/nonexistent/photo.jpg").unwrap(),
        src.stamp(),
    );
    let result = decode(&missing, &sniffed);
    assert!(matches!(result, Err(ImageError::Read { .. })), "{result:?}");
}

#[cfg(feature = "avif")]
#[test]
fn an_avif_decodes_with_the_avif_feature() {
    let picture = still("photo.avif");
    assert_eq!(picture.size(), size(48, 32));
}

#[cfg(not(feature = "avif"))]
#[test]
fn an_avif_says_the_feature_is_missing_without_it() {
    let (src, sniffed) = fixture("photo.avif");
    assert_eq!(
        decode(&src, &sniffed),
        Err(ImageError::NotCompiledIn {
            format: anyview_core::RasterFormat::Avif
        })
    );
}

#[test]
fn the_size_a_header_declares_is_the_size_decoding_gives_upright() {
    // name, whether the header route reads this format
    const CASES: &[(&str, bool)] = &[
        ("quadrants.png", true),
        ("plain.jpg", true),
        ("rotated.jpg", true),
        ("spin.gif", true),
        ("anim.webp", true),
        ("lossy.webp", true),
        ("photo.jxl", false),
        ("logo.svg", false),
    ];
    for (name, read) in CASES {
        let (src, sniffed) = fixture(name);
        let declared = declared_size(&src, &sniffed).unwrap();
        if !read {
            assert_eq!(declared, None, "{name}: size comes from decoding");
            continue;
        }
        let decoded = match decode(&src, &sniffed).unwrap() {
            Decoded::Still(picture) => picture.size(),
            Decoded::Animated(animation) => animation.frames.first().pixels.size(),
            Decoded::HeldStill { picture, .. } => picture.size(),
        };
        assert_eq!(declared, Some(decoded), "{name}");
    }
}
