//! Writing the orientation tag into a raw EXIF (TIFF) block without disturbing the rest of it.
//!
//! `kamadak-exif` can only write a block from scratch, which would drop the maker notes and every
//! tag it does not model. The orientation entry is a fixed-size field of IFD0, so it is patched in
//! place; when the block has none, IFD0 is copied to the end of the block with the entry added, so
//! no existing offset moves.

use crate::error::ImageError;
use crate::orientation::ExifOrientation;

const ORIENTATION_TAG: u16 = 0x0112;
const SHORT: u16 = 3;
const ENTRY_LEN: usize = 12;

/// How the block's numbers are written.
#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, bytes: &[u8], at: usize) -> Option<u16> {
        let pair: [u8; 2] = bytes.get(at..at + 2)?.try_into().ok()?;
        Some(match self {
            Endian::Little => u16::from_le_bytes(pair),
            Endian::Big => u16::from_be_bytes(pair),
        })
    }

    fn u32(self, bytes: &[u8], at: usize) -> Option<u32> {
        let quad: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
        Some(match self {
            Endian::Little => u32::from_le_bytes(quad),
            Endian::Big => u32::from_be_bytes(quad),
        })
    }

    fn put_u16(self, value: u16) -> [u8; 2] {
        match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        }
    }

    fn put_u32(self, value: u32) -> [u8; 4] {
        match self {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        }
    }
}

fn malformed(reason: &'static str) -> ImageError {
    ImageError::Exif { reason }
}

/// `tiff` with its orientation entry set to `orientation`: the same bytes except two, or, when the
/// block has no orientation entry, the same bytes followed by a copy of IFD0 that has one.
pub(crate) fn with_orientation(
    tiff: &[u8],
    orientation: ExifOrientation,
) -> Result<Vec<u8>, ImageError> {
    let endian = match tiff.get(..4) {
        Some(b"II*\0") => Endian::Little,
        Some(b"MM\0*") => Endian::Big,
        Some(_) | None => return Err(malformed("not a TIFF header")),
    };
    let ifd0 = endian
        .u32(tiff, 4)
        .and_then(|offset| usize::try_from(offset).ok())
        .ok_or(malformed("no IFD0 offset"))?;
    let count = usize::from(endian.u16(tiff, ifd0).ok_or(malformed("IFD0 is cut off"))?);
    let entries = ifd0 + 2;
    let end = entries + count * ENTRY_LEN;
    if tiff.len() < end + 4 {
        return Err(malformed("IFD0 is cut off"));
    }
    let tag_at = |index: usize| endian.u16(tiff, entries + index * ENTRY_LEN);
    let value = endian.put_u16(orientation.tag());

    let found = (0..count).find(|&i| tag_at(i) == Some(ORIENTATION_TAG));
    match found {
        Some(index) => {
            let entry = entries + index * ENTRY_LEN;
            let is_short = endian.u16(tiff, entry + 2) == Some(SHORT);
            if !is_short || endian.u32(tiff, entry + 4) != Some(1) {
                return Err(malformed("the orientation entry is not one short"));
            }
            let mut patched = tiff.to_vec();
            patched[entry + 8..entry + 10].copy_from_slice(&value);
            Ok(patched)
        }
        None => Ok(appended(tiff, endian, (entries, count), value)),
    }
}

/// `tiff` plus a new IFD0 holding every old entry and an orientation entry, sorted by tag, with
/// the header pointing at it.
fn appended(tiff: &[u8], endian: Endian, ifd: (usize, usize), value: [u8; 2]) -> Vec<u8> {
    let (entries, count) = ifd;
    let mut out = tiff.to_vec();
    if out.len() % 2 == 1 {
        out.push(0); // IFDs start on a word boundary
    }
    let new_offset = out.len();
    let next = &tiff[entries + count * ENTRY_LEN..entries + count * ENTRY_LEN + 4];
    let position = (0..count)
        .find(|&i| {
            endian
                .u16(tiff, entries + i * ENTRY_LEN)
                .is_some_and(|t| t > ORIENTATION_TAG)
        })
        .unwrap_or(count);
    out.extend_from_slice(&endian.put_u16(u16::try_from(count + 1).unwrap_or(u16::MAX)));
    out.extend_from_slice(&tiff[entries..entries + position * ENTRY_LEN]);
    out.extend_from_slice(&endian.put_u16(ORIENTATION_TAG));
    out.extend_from_slice(&endian.put_u16(SHORT));
    out.extend_from_slice(&endian.put_u32(1));
    out.extend_from_slice(&value);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&tiff[entries + position * ENTRY_LEN..entries + count * ENTRY_LEN]);
    out.extend_from_slice(next);
    let pointer = u32::try_from(new_offset).unwrap_or(u32::MAX); // a block is at most 64 KiB
    out[4..8].copy_from_slice(&endian.put_u32(pointer));
    out
}
