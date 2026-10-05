//! The picture a camera raw file carries inside it: a full-size or near full-size JPEG that the
//! camera wrote beside the sensor data, shown in place of a raw development.
//!
//! Most raw formats (CR2, NEF, ARW, DNG, ORF, RW2, PEF, SRW…) are TIFF files and Canon's CR3 is an
//! ISO base media file, and every one of them stores the preview as a plain JPEG stream somewhere
//! in the file (in a TIFF IFD, a maker note or a box). Which tag points at it differs by maker and
//! by model, so this does not follow the tags: it finds every baseline or progressive JPEG stream
//! in the file by its markers, checks each by walking its segments to the end, and takes the one
//! with the most pixels. The sensor data itself is lossless JPEG (or something else), which the
//! walk refuses by its frame type, so it is never taken for a preview.
//!
//! What is covered: every file that holds such a stream, which is every TIFF-based raw the viewer
//! sniffs and CR3. What is not: a raw whose only preview is not JPEG (some compressed or
//! proprietary previews), and anything in a container that stores the JPEG split in pieces. Those
//! show no picture; the raw plugin (`anyview-raw`) develops them in full.
//!
//! Orientation: the preview's own EXIF orientation when it has one, else the orientation in the
//! raw file's first IFD, which describes the sensor picture the preview was made from.

use super::stills;
use crate::error::ImageError;
use crate::orientation::ExifOrientation;
use crate::pixels::Rgba8;
use anyview_core::{PixelLen, PixelSize};
use image::ImageFormat;
use std::ops::Range;

/// A JPEG stream found in a raw file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preview {
    /// Where the stream lies in the file.
    pub at: Range<usize>,
    /// The size the stream declares, before any orientation.
    pub size: PixelSize,
}

impl Preview {
    fn area(&self) -> u64 {
        self.size.area().0
    }
}

/// The largest decodable JPEG stream in `bytes`, or `None` when there is none.
pub(crate) fn largest_preview(bytes: &[u8]) -> Option<Preview> {
    let mut best: Option<Preview> = None;
    let mut at = 0;
    while let Some(found) = next_start(bytes, at) {
        match stream_at(bytes, found) {
            Some(preview) => {
                at = preview.at.end;
                // The first of two equal streams wins, so the result does not depend on later bytes.
                if best.as_ref().is_none_or(|b| preview.area() > b.area()) {
                    best = Some(preview);
                }
            }
            None => at = found + 3,
        }
    }
    best
}

/// The first place at or after `from` where a JPEG stream may start: `FF D8 FF`.
fn next_start(bytes: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(offset) = bytes.get(at..)?.iter().position(|b| *b == 0xFF) {
        let found = at + offset;
        if bytes.get(found + 1) == Some(&0xD8) && bytes.get(found + 2) == Some(&0xFF) {
            return Some(found);
        }
        at = found + 1;
    }
    None
}

/// Walks the JPEG stream that starts at `start`: its segments, then its entropy-coded data, to the
/// end-of-image marker. `None` when it is cut short, has no frame header, or is a frame type the
/// decoder does not read (lossless, arithmetic-coded, hierarchical, or not 8 bits deep).
fn stream_at(bytes: &[u8], start: usize) -> Option<Preview> {
    let data = bytes.get(start..)?;
    let mut at = 2;
    let mut size: Option<PixelSize> = None;
    loop {
        // Fill bytes may precede a marker.
        while *data.get(at)? == 0xFF && *data.get(at + 1)? == 0xFF {
            at += 1;
        }
        if *data.get(at)? != 0xFF {
            return None;
        }
        let marker = *data.get(at + 1)?;
        at += 2;
        match marker {
            0xD9 => {
                return size.map(|size| Preview {
                    at: start..start + at,
                    size,
                });
            }
            // Markers with no length: a restart, the temporary marker, a stray start of image.
            0x01 | 0xD0..=0xD8 => continue,
            _ => {}
        }
        let length = usize::from(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]));
        let segment = data.get(at..at + length)?;
        match marker {
            // Baseline, extended sequential and progressive Huffman frames.
            0xC0..=0xC2 => {
                let precision = *segment.get(2)?;
                let height = u16::from_be_bytes([*segment.get(3)?, *segment.get(4)?]);
                let width = u16::from_be_bytes([*segment.get(5)?, *segment.get(6)?]);
                if precision != 8 || width == 0 || height == 0 {
                    return None;
                }
                size = Some(PixelSize {
                    width: PixelLen(u32::from(width)),
                    height: PixelLen(u32::from(height)),
                });
            }
            // Lossless, differential, arithmetic-coded frames: not a preview we can show.
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            _ => {}
        }
        at += length;
        if marker == 0xDA {
            at = end_of_scan(data, at)?;
        }
    }
}

/// Where the entropy-coded data that starts at `from` ends: at the next marker that is not a
/// stuffed zero or a restart.
fn end_of_scan(data: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    loop {
        at += data.get(at..)?.iter().position(|b| *b == 0xFF)?;
        match *data.get(at + 1)? {
            0x00 | 0xD0..=0xD7 => at += 2,
            0xFF => at += 1,
            _ => return Some(at),
        }
    }
}

/// The orientation in the first IFD of a TIFF-based raw file, when there is one. Reads the
/// header's byte order and the one tag, nothing else, so it also reads the files whose header
/// carries the maker's own version number (Olympus `IIRO`, Panasonic `II\x55\0`).
pub(crate) fn file_orientation(bytes: &[u8]) -> Option<ExifOrientation> {
    let big = match bytes.get(..2)? {
        b"II" => false,
        b"MM" => true,
        _ => return None,
    };
    let short = |at: usize| -> Option<u16> {
        let pair = [*bytes.get(at)?, *bytes.get(at + 1)?];
        Some(if big {
            u16::from_be_bytes(pair)
        } else {
            u16::from_le_bytes(pair)
        })
    };
    let long = |at: usize| -> Option<u32> {
        let quad = [
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ];
        Some(if big {
            u32::from_be_bytes(quad)
        } else {
            u32::from_le_bytes(quad)
        })
    };
    let ifd = usize::try_from(long(4)?).ok()?;
    let count = usize::from(short(ifd)?);
    (0..count).find_map(|entry| {
        let at = ifd + 2 + entry * 12;
        // Tag 0x0112, type SHORT, one value stored in the entry itself.
        (short(at)? == 0x0112 && short(at + 2)? == 3 && long(at + 4)? == 1)
            .then(|| short(at + 8))
            .flatten()
            .and_then(ExifOrientation::from_tag)
    })
}

/// The preview of a raw file as an upright picture.
pub(crate) fn decode(bytes: &[u8]) -> Result<Rgba8, ImageError> {
    let preview = largest_preview(bytes).ok_or(ImageError::NoPreview)?;
    let jpeg = &bytes[preview.at];
    let (picture, _) = stills::still(jpeg, ImageFormat::Jpeg)?;
    // `still` has turned the picture by the preview's own orientation, when it has one.
    let own = crate::exif::ExifFacts::has_orientation(jpeg);
    Ok(match (own, file_orientation(bytes)) {
        (false, Some(file)) => file.applied(&picture),
        (true, _) | (false, None) => picture,
    })
}

#[cfg(test)]
mod tests;
