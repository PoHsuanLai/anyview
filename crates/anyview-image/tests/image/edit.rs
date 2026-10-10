//! Edits saved in place: a JPEG changes only its orientation tag, a lossless format is written
//! again turned, and a format that would lose something is refused.

#![cfg(feature = "encode")]

use crate::support;

use anyview_core::{
    Adjust, Axis, Edit, EditKind, PageIndex, PageRange, PixelLen, PixelRect, PixelSize,
    QuarterTurn, RasterFormat, Reflection,
};
use anyview_image::{Decoded, ExifFacts, ImageError, Location, Rgba8, decode_bytes, edited};
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
            "a cut animated gif",
            "spin.gif",
            Edit::Adjust(cut_to(PixelRect {
                left: PixelLen(0),
                top: PixelLen(0),
                size: PixelSize {
                    width: PixelLen(1),
                    height: PixelLen(1),
                },
            })),
            ImageError::NotSavable {
                format: RasterFormat::Gif,
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

/// An adjustment that only cuts to `rect`.
fn cut_to(rect: PixelRect) -> Adjust {
    Adjust {
        crop: Some(rect),
        ..Adjust::NONE
    }
}

fn size_of(picture: &Rgba8) -> (u32, u32) {
    (picture.size().width.0, picture.size().height.0)
}

#[test]
fn a_cut_mirrored_and_turned_png_is_the_cut_of_the_pixels_mirrored_and_turned() {
    let before = bytes("quadrants.png");
    let original = still(&before, "a.png");
    let (width, height) = size_of(&original);
    let rect = PixelRect {
        left: PixelLen(0),
        top: PixelLen(0),
        size: PixelSize {
            width: PixelLen(width / 2),
            height: PixelLen(height),
        },
    };
    let adjust = Adjust {
        crop: Some(rect),
        reflection: Reflection::Mirrored,
        turn: QuarterTurn::Quarter,
        size: None,
    };
    let after = edited(&before, &sniffed(&before, "a.png"), Edit::Adjust(adjust)).unwrap();
    let cut = imageops::crop_imm(&image_of(&original), 0, 0, width / 2, height).to_image();
    let expected = imageops::rotate90(&imageops::flip_horizontal(&cut));
    assert_eq!(
        still(&after, "a.png").bytes(),
        expected.as_raw().as_slice(),
        "cut first, then mirrored, then turned"
    );
}

#[test]
fn a_resized_picture_is_saved_at_the_size_chosen_in_its_own_format() {
    // name, file, the size asked for
    let cases = [
        ("a png", "quadrants.png", "a.png"),
        ("a jpeg", "plain.jpg", "a.jpg"),
    ];
    for (name, file, called) in cases {
        let before = bytes(file);
        let original = still(&before, called);
        let (width, height) = size_of(&original);
        let wanted = PixelSize {
            width: PixelLen((width / 2).max(1)),
            height: PixelLen((height / 2).max(1)),
        };
        let adjust = Adjust {
            size: Some(wanted),
            ..Adjust::NONE
        };
        let after = edited(&before, &sniffed(&before, called), Edit::Adjust(adjust)).unwrap();
        assert_eq!(
            sniffed(&after, called).detail(),
            sniffed(&before, called).detail(),
            "{name}: still the same format"
        );
        assert_eq!(still(&after, called).size(), wanted, "{name}");
    }
}

#[test]
fn a_cut_jpeg_has_its_turn_in_the_pixels_and_an_upright_tag() {
    let before = bytes("rotated.jpg");
    let original = still(&before, "a.jpg");
    let (width, height) = size_of(&original);
    assert_eq!(ExifFacts::read(&before).orientation.tag(), 6);
    let adjust = cut_to(PixelRect {
        left: PixelLen(0),
        top: PixelLen(0),
        size: PixelSize {
            width: PixelLen((width / 2).max(1)),
            height: PixelLen((height / 2).max(1)),
        },
    });
    let after = edited(&before, &sniffed(&before, "a.jpg"), Edit::Adjust(adjust)).unwrap();
    assert_eq!(
        ExifFacts::read(&after).orientation.tag(),
        1,
        "the orientation is baked in, so the tag is upright"
    );
    assert_eq!(
        size_of(&still(&after, "a.jpg")),
        ((width / 2).max(1), (height / 2).max(1))
    );
}

#[test]
fn a_turn_and_a_mirror_alone_are_the_lossless_edits_and_nothing_is_the_file_itself() {
    let before = bytes("rotated.jpg");
    let kind = sniffed(&before, "a.jpg");
    let turned = Adjust::NONE.turned(QuarterTurn::Quarter);
    assert_eq!(
        edited(&before, &kind, Edit::Adjust(turned)).unwrap(),
        edited(&before, &kind, Edit::Rotate(QuarterTurn::Quarter)).unwrap(),
        "a turn alone changes only the tag"
    );
    assert_eq!(
        edited(&before, &kind, Edit::Adjust(Adjust::NONE)).unwrap(),
        before,
        "nothing done writes the file as it is"
    );
}

#[test]
fn a_series_of_turns_and_flips_folded_into_one_adjustment_places_the_pixels_as_the_series_does() {
    let before = bytes("quadrants.png");
    let kind = sniffed(&before, "a.png");
    let steps = [
        Edit::Rotate(QuarterTurn::Quarter),
        Edit::Flip(Axis::Horizontal),
        Edit::Rotate(QuarterTurn::ThreeQuarter),
        Edit::Flip(Axis::Vertical),
        Edit::Rotate(QuarterTurn::Half),
    ];
    let mut one_by_one = before.clone();
    let mut folded = Adjust::NONE;
    for step in steps {
        one_by_one = edited(&one_by_one, &kind, step).unwrap();
        folded = match step {
            Edit::Rotate(by) => folded.turned(by),
            Edit::Flip(axis) => folded.flipped(axis),
            Edit::DeletePages(_) | Edit::MovePage { .. } | Edit::Adjust(_) => folded,
        };
        let at_once = edited(&before, &kind, Edit::Adjust(folded)).unwrap();
        assert_eq!(
            still(&at_once, "a.png"),
            still(&one_by_one, "a.png"),
            "after {step:?}"
        );
    }
}

#[test]
fn a_cut_keeps_the_place_a_photo_had_and_never_gives_one_that_had_none() {
    // name, file, whether the original says where it was taken
    const CASES: &[(&str, &str, bool)] = &[
        ("a located photo", "located.jpg", true),
        ("a located PNG", "located.png", true),
        ("a photo with no place", "plain.jpg", false),
    ];
    for (name, file, located) in CASES {
        let before = bytes(file);
        assert_eq!(
            Location::of_file(&before).is_some(),
            *located,
            "{name}: fixture"
        );
        let (width, height) = size_of(&still(&before, file));
        let adjust = cut_to(PixelRect {
            left: PixelLen(0),
            top: PixelLen(0),
            size: PixelSize {
                width: PixelLen((width / 2).max(1)),
                height: PixelLen((height / 2).max(1)),
            },
        });
        let after = edited(&before, &sniffed(&before, file), Edit::Adjust(adjust)).unwrap();
        assert_eq!(
            Location::of_file(&after).is_some(),
            *located,
            "{name}: after the cut"
        );
    }
}
