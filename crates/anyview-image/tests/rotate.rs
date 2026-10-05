//! Lossless rotation: only the EXIF segment changes, never the image data.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{PixelLen, PixelSize, QuarterTurn};
use anyview_image::{Decoded, ExifFacts, ImageError, decode_bytes, rotate_jpeg};
use std::ops::Range;
use support::{bytes, sniffed};

/// The byte range of the first APP1 segment: marker, length and payload.
fn app1(file: &[u8]) -> Range<usize> {
    let mut at = 2;
    while at + 4 <= file.len() {
        assert_eq!(file[at], 0xFF, "a marker at {at}");
        let length = usize::from(u16::from_be_bytes([file[at + 2], file[at + 3]]));
        if file[at + 1] == 0xE1 {
            return at..at + 2 + length;
        }
        at += 2 + length;
    }
    panic!("no APP1 segment");
}

fn without(file: &[u8], range: Range<usize>) -> Vec<u8> {
    [&file[..range.start], &file[range.end..]].concat()
}

fn tag(file: &[u8]) -> u16 {
    ExifFacts::read(file).orientation.tag()
}

fn upright_size(file: &[u8]) -> PixelSize {
    match decode_bytes(file, &sniffed(file, "a.jpg")).unwrap() {
        Decoded::Still(picture) => picture.size(),
        Decoded::Animated(_) | Decoded::HeldStill { .. } => panic!("a jpeg is a still"),
    }
}

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

#[test]
fn turning_a_photo_with_an_orientation_changes_one_byte_of_its_exif_and_nothing_else() {
    let before = bytes("rotated.jpg");
    assert_eq!(tag(&before), 6);
    let after = rotate_jpeg(&before, QuarterTurn::Quarter).unwrap();
    assert_eq!(tag(&after), 3, "6 turned a quarter is 3");
    assert_eq!(after.len(), before.len());
    let segment = app1(&before);
    assert_eq!(
        app1(&after),
        segment,
        "the segment keeps its place and length"
    );
    assert_eq!(without(&after, segment.clone()), without(&before, segment));
    let changed = (0..before.len()).filter(|&i| before[i] != after[i]).count();
    assert_eq!(changed, 1, "only the orientation's low byte differs");
}

#[test]
fn four_quarter_turns_return_the_original_bytes() {
    let original = bytes("rotated.jpg");
    let mut file = original.clone();
    for _ in 0..4 {
        file = rotate_jpeg(&file, QuarterTurn::Quarter).unwrap();
    }
    assert_eq!(file, original);
}

#[test]
fn a_turn_changes_what_decoding_shows() {
    let original = bytes("rotated.jpg");
    assert_eq!(upright_size(&original), size(32, 48));
    let quarter = rotate_jpeg(&original, QuarterTurn::Quarter).unwrap(); // now orientation 3: 48 x 32
    assert_eq!(upright_size(&quarter), size(48, 32));
    let back = rotate_jpeg(&quarter, QuarterTurn::ThreeQuarter).unwrap();
    assert_eq!(upright_size(&back), size(32, 48));
}

#[test]
fn a_photo_without_exif_gains_a_segment_and_keeps_every_other_byte() {
    let before = bytes("plain.jpg");
    assert_eq!(ExifFacts::read(&before), ExifFacts::none());
    let after = rotate_jpeg(&before, QuarterTurn::Quarter).unwrap();
    assert_eq!(tag(&after), 6);
    assert_eq!(upright_size(&after), size(32, 48));
    assert_eq!(
        without(&after, app1(&after)),
        before,
        "the original is intact around the new segment"
    );
    // The segment goes after the JFIF header, which stays first.
    assert_eq!(before[2..4], [0xFF, 0xE0], "the fixture has a JFIF header");
    assert_eq!(after[2..4], [0xFF, 0xE0]);
    assert!(app1(&after).start > 2);
}

#[test]
fn bytes_after_the_end_of_image_survive() {
    let mut file = bytes("rotated.jpg");
    file.extend_from_slice(b"MPF-TRAILER");
    let after = rotate_jpeg(&file, QuarterTurn::Half).unwrap();
    assert!(after.ends_with(b"MPF-TRAILER"));
    assert_eq!(tag(&after), 8, "6 turned a half is 8");
}

#[test]
fn no_turn_returns_the_file_as_it_was() {
    let file = bytes("rotated.jpg");
    assert_eq!(rotate_jpeg(&file, QuarterTurn::None).unwrap(), file);
}

#[test]
fn a_file_that_is_not_a_jpeg_is_refused() {
    const CASES: &[(&str, &[u8])] = &[
        ("png", b"\x89PNG\r\n\x1a\n"),
        ("empty", b""),
        ("a marker with no length", b"\xFF\xD8\xFF\xE1\x00"),
        ("a segment past the end", b"\xFF\xD8\xFF\xE1\x00\x40ab"),
    ];
    for (name, file) in CASES {
        assert!(
            matches!(
                rotate_jpeg(file, QuarterTurn::Quarter),
                Err(ImageError::Container { .. })
            ),
            "{name}"
        );
    }
}
