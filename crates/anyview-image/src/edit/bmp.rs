//! A Windows bitmap turned or mirrored at the depth it had: 24-bit stays 24-bit, 32-bit stays
//! 32-bit, and an 8-bit bitmap keeps its palette. The resolution is kept and its axes swapped
//! after a quarter turn. Other depths and compressed bitmaps are not offered.

use super::place::Grid;
use super::{Fidelity, Loss, Placing};
use crate::error::ImageError;
use crate::pixels::Rgba8;
use image::ExtendedColorType;
use image::codecs::bmp::BmpEncoder;

/// What the headers say.
struct Shape {
    width: u32,
    height: u32,
    bits: u16,
    pixels_at: usize,
    ppm: [[u8; 4]; 2],
    palette: Vec<[u8; 3]>,
}

fn le32(file: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(file.get(at..at + 4)?.try_into().ok()?))
}

fn shape_of(file: &[u8]) -> Option<Shape> {
    if file.get(..2)? != b"BM" {
        return None;
    }
    let dib = usize::try_from(le32(file, 14)?).ok()?;
    let width = le32(file, 18)?;
    let height = i32::from_le_bytes(file.get(22..26)?.try_into().ok()?);
    let bits = u16::from_le_bytes(file.get(28..30)?.try_into().ok()?);
    let compression = le32(file, 30)?;
    let used = usize::try_from(le32(file, 46)?).ok()?;
    let table = 14 + dib;
    let colours = if used == 0 { 256 } else { used.min(256) };
    let palette = if bits == 8 {
        file.get(table..table + colours * 4)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| [c[2], c[1], c[0]])
            .collect()
    } else {
        Vec::new()
    };
    let ok = dib >= 40
        && compression == 0
        && matches!(bits, 8 | 24 | 32)
        && height > 0 // a top-down bitmap is stored the other way up
        && width > 0;
    if !ok {
        return None;
    }
    Some(Shape {
        width,
        height: height.unsigned_abs(),
        bits,
        pixels_at: usize::try_from(le32(file, 10)?).ok()?,
        ppm: [
            file.get(38..42)?.try_into().ok()?,
            file.get(42..46)?.try_into().ok()?,
        ],
        palette,
    })
}

/// What turning the bitmap costs: nothing for the depths kept, colour detail for the others.
pub(super) fn fidelity(file: &[u8]) -> Fidelity {
    if shape_of(file).is_some() {
        Fidelity::Intact
    } else if file.starts_with(b"BM") {
        Fidelity::Loses(Loss::Colour)
    } else {
        Fidelity::Impossible
    }
}

fn encode_error(error: image::ImageError) -> ImageError {
    ImageError::Encode {
        reason: error.to_string(),
    }
}

/// The palette indices of an 8-bit bitmap, top row first.
fn indices(file: &[u8], shape: &Shape) -> Result<Grid, ImageError> {
    let (w, h) = (shape.width as usize, shape.height as usize);
    let line = w.div_ceil(4) * 4;
    let rows = file
        .get(shape.pixels_at..shape.pixels_at + line * h)
        .ok_or_else(|| ImageError::Container {
            reason: "the bitmap is cut off".to_owned(),
        })?;
    let bytes = rows
        .chunks_exact(line)
        .rev()
        .flat_map(|row| row[..w].iter().copied())
        .collect();
    Ok(Grid {
        width: shape.width,
        height: shape.height,
        unit: 1,
        bytes,
    })
}

/// The bitmap with its resolution set to the original's, axes swapped when `swap`.
fn with_resolution(mut out: Vec<u8>, ppm: [[u8; 4]; 2], swap: bool) -> Vec<u8> {
    let [x, y] = if swap { [ppm[1], ppm[0]] } else { ppm };
    if out.len() >= 46 {
        out[38..42].copy_from_slice(&x);
        out[42..46].copy_from_slice(&y);
    }
    out
}

/// `file` with `placing` applied; `moved` is its picture already placed, which a 24 or 32-bit
/// bitmap is written from.
pub(super) fn rewritten(
    file: &[u8],
    placing: Placing,
    moved: &Rgba8,
) -> Result<Vec<u8>, ImageError> {
    let shape = shape_of(file).ok_or_else(|| ImageError::Container {
        reason: "not a bitmap this writer keeps".to_owned(),
    })?;
    let swap = placing.swaps();
    let mut out = Vec::new();
    let mut encoder = BmpEncoder::new(&mut out);
    match shape.bits {
        8 => {
            let grid = indices(file, &shape)?
                .placed(placing.over(crate::orientation::ExifOrientation::UPRIGHT));
            encoder
                .encode_with_palette(
                    &grid.bytes,
                    grid.width,
                    grid.height,
                    ExtendedColorType::L8,
                    Some(&shape.palette),
                )
                .map_err(encode_error)?;
        }
        24 => {
            let size = moved.size();
            let rgb: Vec<u8> = moved
                .bytes()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|px| [px[0], px[1], px[2]])
                .collect();
            encoder
                .encode(&rgb, size.width.0, size.height.0, ExtendedColorType::Rgb8)
                .map_err(encode_error)?;
        }
        _ => {
            let size = moved.size();
            encoder
                .encode(
                    moved.bytes(),
                    size.width.0,
                    size.height.0,
                    ExtendedColorType::Rgba8,
                )
                .map_err(encode_error)?;
        }
    }
    Ok(with_resolution(out, shape.ppm, swap))
}
