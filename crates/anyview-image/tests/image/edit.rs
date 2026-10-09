//! Edits saved in place: a JPEG changes only its orientation tag, a lossless format is written
//! again turned, and a format that would lose something is refused.

#![cfg(feature = "encode")]

use crate::support;

use anyview_core::{Axis, Edit, EditKind, PageIndex, PageRange, QuarterTurn, RasterFormat};
use anyview_image::{Decoded, ExifFacts, ImageError, Rgba8, decode_bytes, edited};
use image::{RgbaImage, imageops};
use support::{bytes, sniffed};

fn still(file: &[u8], name: &str) -> Rgba8 {
    match decode_bytes(file, &sniffed(file, name)).unwrap() {
        Decoded::Still(picture) => picture,
        Decoded::Animated(_) | Decoded::HeldStill { .. } => panic!("{name} is a still"),
    }
}

fn differing_bytes(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len(), "an orientation edit keeps the length");
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

#[test]
fn a_jpeg_turn_changes_only_the_orientation_tag() {
    let before = bytes("rotated.jpg");
    let after = edited(
        &before,
        &sniffed(&before, "a.jpg"),
        Edit::Rotate(QuarterTurn::Quarter),
    )
    .unwrap();
    assert_eq!(ExifFacts::read(&before).orientation.tag(), 6);
    assert_eq!(ExifFacts::read(&after).orientation.tag(), 3);
    assert_eq!(differing_bytes(&before, &after), 1);
}

#[test]
fn a_jpeg_flip_changes_only_the_orientation_tag_and_twice_restores_it() {
    let before = bytes("rotated.jpg");
    let kind = sniffed(&before, "a.jpg");
    let once = edited(&before, &kind, Edit::Flip(Axis::Horizontal)).unwrap();
    assert_eq!(ExifFacts::read(&once).orientation.tag(), 5, "6 mirrored");
    assert_eq!(differing_bytes(&before, &once), 1);
    let twice = edited(&once, &kind, Edit::Flip(Axis::Horizontal)).unwrap();
    assert_eq!(twice, before);
    let down = edited(&before, &kind, Edit::Flip(Axis::Vertical)).unwrap();
    assert_eq!(
        still(&down, "a.jpg").bytes(),
        imageops::flip_vertical(&image_of(&still(&before, "a.jpg"))).as_raw(),
        "what decoding shows is the rows reversed"
    );
}

/// The picture as the `image` crate holds it, to compare with what its own operations make.
fn image_of(picture: &Rgba8) -> RgbaImage {
    let size = picture.size();
    RgbaImage::from_raw(size.width.0, size.height.0, picture.bytes().to_vec()).unwrap()
}

#[test]
fn a_png_turn_decodes_turned_and_keeps_its_size_swapped() {
    let before = bytes("quadrants.png");
    let kind = sniffed(&before, "a.png");
    let original = still(&before, "a.png");
    let after = edited(&before, &kind, Edit::Rotate(QuarterTurn::Quarter)).unwrap();
    let turned = still(&after, "a.png");
    let expected = imageops::rotate90(&image_of(&original));
    assert_eq!(
        (turned.size().width.0, turned.size().height.0),
        expected.dimensions()
    );
    assert_eq!(turned.bytes(), expected.as_raw().as_slice());
    assert_ne!(
        turned.bytes(),
        original.bytes(),
        "the edit made a difference"
    );
}

#[test]
fn a_png_flip_decodes_mirrored() {
    let before = bytes("quadrants.png");
    let original = still(&before, "a.png");
    let after = edited(
        &before,
        &sniffed(&before, "a.png"),
        Edit::Flip(Axis::Horizontal),
    )
    .unwrap();
    let expected = imageops::flip_horizontal(&image_of(&original));
    assert_eq!(still(&after, "a.png").bytes(), expected.as_raw().as_slice());
}

#[test]
fn a_still_webp_is_written_again_as_webp() {
    let before = bytes("lossy.webp");
    let original = still(&before, "a.webp");
    let after = edited(
        &before,
        &sniffed(&before, "a.webp"),
        Edit::Rotate(QuarterTurn::Half),
    )
    .unwrap();
    let expected = imageops::rotate180(&image_of(&original));
    assert_eq!(
        still(&after, "a.webp").bytes(),
        expected.as_raw().as_slice()
    );
    assert_eq!(still(&after, "a.webp").size(), original.size());
}

#[test]
fn what_would_lose_something_is_refused() {
    let turn = Edit::Rotate(QuarterTurn::Quarter);
    let pages = Edit::DeletePages(PageRange::new(PageIndex(0), PageIndex(0)).unwrap());
    let cases: &[(&str, &str, Edit, ImageError)] = &[
        (
            "an animated gif",
            "spin.gif",
            turn,
            ImageError::NotSavable {
                format: RasterFormat::Gif,
            },
        ),
        (
            "a jpeg xl picture",
            "photo.jxl",
            turn,
            ImageError::NotSavable {
                format: RasterFormat::Jxl,
            },
        ),
        (
            "pages of a picture",
            "quadrants.png",
            pages,
            ImageError::NotAnImageEdit {
                kind: EditKind::DeletePages,
            },
        ),
    ];
    for (name, file, edit, want) in cases {
        let file_bytes = bytes(file);
        let got = edited(&file_bytes, &sniffed(&file_bytes, file), *edit).unwrap_err();
        assert_eq!(&got, want, "{name}");
    }
}
