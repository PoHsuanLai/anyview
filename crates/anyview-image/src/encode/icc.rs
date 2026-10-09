//! Adding an ICC profile to an AVIF file that has none.
//!
//! `ravif` writes the colour as numbers (`nclx`) and cannot embed a profile, so the finished file
//! gets a `colr` property of type `prof`, ahead of the others in the primary picture's list, the
//! way libavif and Preview read it. Everything that grows is a box on the way from the file's top
//! to the property list, and the item locations (absolute offsets into `mdat`) move with it.

use crate::error::ImageError;

/// A box: where it starts and where it ends.
#[derive(Clone, Copy)]
struct Span {
    at: usize,
    end: usize,
}

fn bad(reason: &str) -> ImageError {
    ImageError::Container {
        reason: format!("AVIF: {reason}"),
    }
}

fn be32(file: &[u8], at: usize) -> Result<u32, ImageError> {
    let quad: [u8; 4] = file
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| bad("cut off"))?;
    Ok(u32::from_be_bytes(quad))
}

fn be16(file: &[u8], at: usize) -> Result<u16, ImageError> {
    let pair: [u8; 2] = file
        .get(at..at + 2)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| bad("cut off"))?;
    Ok(u16::from_be_bytes(pair))
}

/// The boxes laid end to end between `from` and `to`, as (type, span).
fn boxes(file: &[u8], from: usize, to: usize) -> Result<Vec<([u8; 4], Span)>, ImageError> {
    let mut found = Vec::new();
    let mut at = from;
    while at < to {
        let size = usize::try_from(be32(file, at)?).map_err(|_| bad("size"))?;
        let kind: [u8; 4] = file
            .get(at + 4..at + 8)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| bad("cut off"))?;
        if size < 8 || at + size > to {
            return Err(bad("a box of an odd size"));
        }
        found.push((kind, Span { at, end: at + size }));
        at += size;
    }
    Ok(found)
}

fn find(list: &[([u8; 4], Span)], kind: &[u8; 4]) -> Result<Span, ImageError> {
    list.iter()
        .find(|(k, _)| k == kind)
        .map(|(_, span)| *span)
        .ok_or_else(|| bad("a box is missing"))
}

fn grow(file: &mut [u8], span: Span, by: usize) -> Result<(), ImageError> {
    let size = u32::try_from(span.end - span.at + by).map_err(|_| bad("too big"))?;
    file[span.at..span.at + 4].copy_from_slice(&size.to_be_bytes());
    Ok(())
}

/// `avif` with `profile` as the colour profile of its primary picture.
pub(super) fn with_profile(mut avif: Vec<u8>, profile: &[u8]) -> Result<Vec<u8>, ImageError> {
    let top = boxes(&avif, 0, avif.len())?;
    let meta = find(&top, b"meta")?;
    let inside = boxes(&avif, meta.at + 12, meta.end)?;
    let (iloc, iprp, pitm) = (
        find(&inside, b"iloc")?,
        find(&inside, b"iprp")?,
        find(&inside, b"pitm")?,
    );
    let parts = boxes(&avif, iprp.at + 8, iprp.end)?;
    let (ipco, ipma) = (find(&parts, b"ipco")?, find(&parts, b"ipma")?);
    if ipco.end > ipma.at || iprp.at < iloc.at {
        return Err(bad("boxes in an unexpected order"));
    }
    let primary = if avif[pitm.at + 8] == 0 {
        u32::from(be16(&avif, pitm.at + 12)?)
    } else {
        be32(&avif, pitm.at + 12)?
    };
    let index = u16::try_from(boxes(&avif, ipco.at + 8, ipco.end)?.len() + 1)
        .map_err(|_| bad("too many properties"))?;

    // The new property box: size, "colr", "prof", the profile.
    let colr_len = 12 + profile.len();
    let wide = avif[ipma.at + 9] & 1 == 1; // the flags: 15-bit property indices
    let assoc_len = if wide { 2 } else { 1 };
    if !wide && index > 127 {
        return Err(bad("too many properties"));
    }
    let delta = colr_len + assoc_len;

    patch_locations(&mut avif, iloc, meta.end, delta)?;

    // The association comes first in the primary picture's list. Later bytes first, so the
    // earlier positions stay true.
    let wide_ids = avif[ipma.at + 8] != 0;
    let entries = usize::try_from(be32(&avif, ipma.at + 12)?).map_err(|_| bad("size"))?;
    let mut at = ipma.at + 16;
    let mut target = None;
    for _ in 0..entries {
        let id = if wide_ids {
            be32(&avif, at)?
        } else {
            u32::from(be16(&avif, at)?)
        };
        let count_at = at + if wide_ids { 4 } else { 2 };
        let count = usize::from(*avif.get(count_at).ok_or_else(|| bad("cut off"))?);
        if id == primary {
            target = Some(count_at);
        }
        at = count_at + 1 + count * assoc_len;
    }
    let count_at = target.ok_or_else(|| bad("the primary picture has no properties"))?;
    let assoc = if wide {
        index.to_be_bytes().to_vec()
    } else {
        vec![index as u8] // below 128, checked above
    };
    avif[count_at] += 1;
    avif.splice(count_at + 1..count_at + 1, assoc);
    grow(&mut avif, ipma, assoc_len)?;

    let mut colr = Vec::with_capacity(colr_len);
    colr.extend_from_slice(
        &u32::try_from(colr_len)
            .map_err(|_| bad("too big"))?
            .to_be_bytes(),
    );
    colr.extend_from_slice(b"colrprof");
    colr.extend_from_slice(profile);
    avif.splice(ipco.end..ipco.end, colr);
    grow(&mut avif, ipco, colr_len)?;
    grow(&mut avif, iprp, delta)?;
    grow(&mut avif, meta, delta)?;
    Ok(avif)
}

/// Every offset in `iloc` that points at or past `meta_end` moves by `delta`.
fn patch_locations(
    avif: &mut [u8],
    iloc: Span,
    meta_end: usize,
    delta: usize,
) -> Result<(), ImageError> {
    let version = avif[iloc.at + 8];
    let sizes = *avif.get(iloc.at + 12).ok_or_else(|| bad("cut off"))?;
    let (offset_size, length_size) = (usize::from(sizes >> 4), usize::from(sizes & 15));
    let more = *avif.get(iloc.at + 13).ok_or_else(|| bad("cut off"))?;
    let (base_size, index_size) = (
        usize::from(more >> 4),
        if version == 0 {
            0
        } else {
            usize::from(more & 15)
        },
    );
    if offset_size != 4 && offset_size != 8 || base_size > 8 {
        return Err(bad("item locations of an odd size"));
    }
    let (mut at, items) = if version < 2 {
        (iloc.at + 16, usize::from(be16(avif, iloc.at + 14)?))
    } else {
        (
            iloc.at + 18,
            usize::try_from(be32(avif, iloc.at + 14)?).map_err(|_| bad("size"))?,
        )
    };
    for _ in 0..items {
        at += if version < 2 { 2 } else { 4 }; // the item id
        if version > 0 {
            at += 2; // the construction method
        }
        at += 2; // the data reference
        let base_at = at;
        at += base_size;
        let extents = usize::from(be16(avif, at)?);
        at += 2;
        for _ in 0..extents {
            at += index_size;
            let (field, width) = if base_size > 0 {
                (base_at, base_size)
            } else {
                (at, offset_size)
            };
            bump(avif, field, width, meta_end, delta)?;
            at += offset_size + length_size;
        }
    }
    Ok(())
}

/// Add `delta` to the big-endian number of `width` bytes at `at`, if it points past `limit`.
fn bump(
    avif: &mut [u8],
    at: usize,
    width: usize,
    limit: usize,
    delta: usize,
) -> Result<(), ImageError> {
    let field = avif.get_mut(at..at + width).ok_or_else(|| bad("cut off"))?;
    let value = field.iter().fold(0u64, |n, b| (n << 8) | u64::from(*b));
    if usize::try_from(value).map_err(|_| bad("offset"))? < limit {
        return Ok(());
    }
    let moved = value + delta as u64; // a delta is a profile's size
    for (i, byte) in field.iter_mut().rev().enumerate() {
        *byte = (moved >> (8 * i)) as u8;
    }
    Ok(())
}
