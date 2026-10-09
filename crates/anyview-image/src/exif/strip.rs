//! Taking the GPS directory out of a raw EXIF (TIFF) block, bytes and all.
//!
//! Removing only the pointer tag would leave the position in the block for anyone who reads the
//! bytes, so the directory's entries and every value stored out of line are zeroed too. Nothing
//! else moves: the other directories keep their offsets, because the pointer entry is removed in
//! place by closing the gap in IFD0.

use super::patch::Endian;

const GPS_POINTER_TAG: u16 = 0x8825;
const ENTRY_LEN: usize = 12;

/// The byte size of one value of an EXIF field type, 0 for a type this does not know.
fn unit(kind: u16) -> usize {
    match kind {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => 0,
    }
}

/// `tiff` without its GPS directory and the IFD0 entry that points at it. A block with no GPS
/// directory, or one that is not a TIFF, comes back as it was.
pub(crate) fn without_location(tiff: &[u8]) -> Vec<u8> {
    let mut out = tiff.to_vec();
    let _ = strip(&mut out);
    out
}

fn strip(tiff: &mut [u8]) -> Option<()> {
    let endian = match tiff.get(..4)? {
        b"II*\0" => Endian::Little,
        b"MM\0*" => Endian::Big,
        _ => return None,
    };
    let ifd0 = usize::try_from(endian.u32(tiff, 4)?).ok()?;
    let count = usize::from(endian.u16(tiff, ifd0)?);
    let entries = ifd0 + 2;
    let end = entries + count * ENTRY_LEN;
    tiff.get(..end + 4)?;
    let at =
        (0..count).find(|&i| endian.u16(tiff, entries + i * ENTRY_LEN) == Some(GPS_POINTER_TAG))?;
    let pointer = entries + at * ENTRY_LEN;
    if let Some(gps) = endian
        .u32(tiff, pointer + 8)
        .and_then(|offset| usize::try_from(offset).ok())
    {
        zero_directory(tiff, endian, gps);
    }
    // Close the gap: later entries and the next-IFD link move up one entry, the freed tail is zero.
    tiff.copy_within(pointer + ENTRY_LEN..end + 4, pointer);
    tiff[end - ENTRY_LEN + 4..end + 4].fill(0);
    let remaining = u16::try_from(count - 1).ok()?;
    tiff[ifd0..ifd0 + 2].copy_from_slice(&endian.put_u16(remaining));
    Some(())
}

/// Zero the directory at `at`: its entries, its count, and the values its entries point to.
fn zero_directory(tiff: &mut [u8], endian: Endian, at: usize) -> Option<()> {
    let count = usize::from(endian.u16(tiff, at)?);
    let entries = at + 2;
    for i in 0..count {
        let entry = entries + i * ENTRY_LEN;
        let kind = endian.u16(tiff, entry + 2)?;
        let items = usize::try_from(endian.u32(tiff, entry + 4)?).ok()?;
        let size = unit(kind).saturating_mul(items);
        if size > 4 {
            let offset = usize::try_from(endian.u32(tiff, entry + 8)?).ok()?;
            if let Some(value) = offset
                .checked_add(size)
                .and_then(|end| tiff.get_mut(offset..end))
            {
                value.fill(0);
            }
        }
    }
    let end = (entries + count * ENTRY_LEN + 4).min(tiff.len());
    tiff[at..end].fill(0);
    Some(())
}
