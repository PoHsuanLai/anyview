//! Lossless JPEG rotation: only the EXIF orientation tag changes. The entropy-coded image data is
//! never decoded or re-encoded, so this is what "save in place" does for a rotated photo.
//!
//! The segment is replaced by scanning the file's own markers rather than through `img-parts`,
//! whose JPEG type stops at the end of image (dropping any trailing data, as some cameras append)
//! and moves the EXIF segment; here every byte outside the replaced APP1 segment is copied as it was.

use crate::error::ImageError;
use crate::exif::{ExifFacts, with_orientation};
use crate::orientation::ExifOrientation;
use anyview_core::{Axis, QuarterTurn};
use std::ops::Range;

const EXIF_PREFIX: &[u8] = b"Exif\0\0";
const APP0: u8 = 0xE0;
const APP1: u8 = 0xE1;
const SOS: u8 = 0xDA;
const EOI: u8 = 0xD9;
/// The most a segment's payload may hold: a 16-bit length that counts itself.
const MAX_PAYLOAD: usize = 65_533;

/// Where a file's EXIF segment is, or where one would go.
enum Layout {
    /// A segment from its marker to its last byte, and the TIFF block inside it.
    Existing {
        segment: Range<usize>,
        tiff: Range<usize>,
    },
    /// No segment: one belongs at this offset, after the start of image and any JFIF segment.
    Absent { insert_at: usize },
}

fn not_jpeg(reason: &str) -> ImageError {
    ImageError::Container {
        reason: reason.to_owned(),
    }
}

fn layout(file: &[u8]) -> Result<Layout, ImageError> {
    if !file.starts_with(&[0xFF, 0xD8]) {
        return Err(not_jpeg("no start-of-image marker"));
    }
    let mut position = 2;
    let mut insert_at = 2;
    while file.get(position) == Some(&0xFF) {
        let marker = *file.get(position + 1).ok_or_else(|| not_jpeg("cut off"))?;
        if marker == 0xFF {
            position += 1; // fill byte
            continue;
        }
        if marker == SOS || marker == EOI {
            break;
        }
        let length = file
            .get(position + 2..position + 4)
            .map(|pair| usize::from(u16::from_be_bytes([pair[0], pair[1]])))
            .filter(|length| *length >= 2)
            .ok_or_else(|| not_jpeg("bad segment length"))?;
        let end = position + 2 + length;
        let payload = file
            .get(position + 4..end)
            .ok_or_else(|| not_jpeg("segment runs past the end"))?;
        if marker == APP1 && payload.starts_with(EXIF_PREFIX) {
            return Ok(Layout::Existing {
                segment: position..end,
                tiff: position + 4 + EXIF_PREFIX.len()..end,
            });
        }
        if marker == APP0 && position == 2 {
            insert_at = end;
        }
        position = end;
    }
    Ok(Layout::Absent { insert_at })
}

/// A TIFF block with an empty IFD0, the seed for a photo that had no EXIF.
fn empty_tiff() -> Vec<u8> {
    let mut block = b"II*\0".to_vec();
    block.extend_from_slice(&8u32.to_le_bytes());
    block.extend_from_slice(&0u16.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes());
    block
}

/// An APP1 segment holding `tiff`.
fn segment_of(tiff: &[u8]) -> Result<Vec<u8>, ImageError> {
    let payload = EXIF_PREFIX.len() + tiff.len();
    if payload > MAX_PAYLOAD {
        return Err(ImageError::Exif {
            reason: "the EXIF block would not fit a JPEG segment",
        });
    }
    let length = u16::try_from(payload + 2).unwrap_or(u16::MAX); // bounded just above
    let mut segment = vec![0xFF, APP1];
    segment.extend_from_slice(&length.to_be_bytes());
    segment.extend_from_slice(EXIF_PREFIX);
    segment.extend_from_slice(tiff);
    Ok(segment)
}

/// `file`, a JPEG, displayed turned clockwise by `turn` on top of whatever orientation it already
/// has. Only the orientation entry of the EXIF block changes (the segment is added when the file
/// has none, and the entry when the block has none); every byte outside that segment is the same.
/// A `QuarterTurn::None` returns the file unchanged.
pub fn rotate_jpeg(file: &[u8], turn: QuarterTurn) -> Result<Vec<u8>, ImageError> {
    if turn == QuarterTurn::None {
        return Ok(file.to_vec());
    }
    reoriented(file, |now| now.turned(turn))
}

/// `file`, a JPEG, displayed mirrored across `axis` on top of whatever orientation it already
/// has, with the same promise as [`rotate_jpeg`]: only the orientation entry changes.
pub fn flip_jpeg(file: &[u8], axis: Axis) -> Result<Vec<u8>, ImageError> {
    reoriented(file, |now| now.flipped(axis))
}

/// `file` with its orientation entry set to what `change` makes of the one it has.
fn reoriented(
    file: &[u8],
    change: impl FnOnce(ExifOrientation) -> ExifOrientation,
) -> Result<Vec<u8>, ImageError> {
    let target = change(ExifFacts::read(file).orientation);
    match layout(file)? {
        Layout::Existing { segment, tiff } => {
            let patched = with_orientation(&file[tiff], target)?;
            Ok(splice(file, segment, &segment_of(&patched)?))
        }
        Layout::Absent { insert_at } => {
            let block = with_orientation(&empty_tiff(), target)?;
            Ok(splice(file, insert_at..insert_at, &segment_of(&block)?))
        }
    }
}

/// `file` with `range` replaced by `with`.
fn splice(file: &[u8], range: Range<usize>, with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(file.len() - range.len() + with.len());
    out.extend_from_slice(&file[..range.start]);
    out.extend_from_slice(with);
    out.extend_from_slice(&file[range.end..]);
    out
}
