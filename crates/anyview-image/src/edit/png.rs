//! A PNG turned or mirrored without being converted: the colour type, bit depth, palette and
//! transparency stay, and every other chunk is copied through (text, XMP, colour profile, gamma,
//! resolution and the rest). Only what the edit makes wrong is rewritten: the size, the
//! resolution's axes after a quarter turn, and an EXIF orientation, which the pixels now carry.

use super::place::{Grid, swaps_axes};
use super::{Fidelity, Loss, Placing};
use crate::error::ImageError;
use crate::exif::{ExifFacts, with_orientation};
use crate::orientation::ExifOrientation;
use png::{BitDepth, ColorType, Decoder, Encoder, Transformations};
use std::io::Cursor;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// One chunk of a file: its type, its data, and every byte of it as written (length to checksum).
struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    raw: &'a [u8],
}

fn container(reason: &str) -> ImageError {
    ImageError::Container {
        reason: reason.to_owned(),
    }
}

/// The chunks of `file` up to its end chunk; `None` when it is cut off or is not a PNG.
fn chunks(file: &[u8]) -> Option<Vec<Chunk<'_>>> {
    if !file.starts_with(&SIGNATURE) {
        return None;
    }
    let mut at = SIGNATURE.len();
    let mut found = Vec::new();
    loop {
        let length =
            usize::try_from(u32::from_be_bytes(file.get(at..at + 4)?.try_into().ok()?)).ok()?;
        let end = at.checked_add(12)?.checked_add(length)?;
        let raw = file.get(at..end)?;
        let kind: [u8; 4] = raw[4..8].try_into().ok()?;
        found.push(Chunk {
            kind,
            data: &raw[8..8 + length],
            raw,
        });
        if &kind == b"IEND" {
            return Some(found);
        }
        at = end;
    }
}

/// The checksum of a chunk's type and data.
fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = !0u32;
    for byte in parts.iter().flat_map(|part| part.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn chunk_bytes(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes()); // a chunk is under 4 GiB
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
    out
}

/// What the header says.
struct Header {
    width: u32,
    height: u32,
    depth: u8,
    colour: u8,
}

fn header_of(chunks: &[Chunk<'_>]) -> Option<Header> {
    let first = chunks.first().filter(|c| &c.kind == b"IHDR")?;
    let d = first.data;
    Some(Header {
        width: u32::from_be_bytes(d.get(0..4)?.try_into().ok()?),
        height: u32::from_be_bytes(d.get(4..8)?.try_into().ok()?),
        depth: *d.get(8)?,
        colour: *d.get(9)?,
    })
}

/// What turning the file costs: nothing, the frames after the first of an animation, or an EXIF
/// block whose orientation cannot be reset.
pub(super) fn fidelity(file: &[u8]) -> Fidelity {
    let Some(chunks) = chunks(file) else {
        return Fidelity::Impossible;
    };
    let animated = chunks
        .iter()
        .any(|c| matches!(&c.kind, b"acTL" | b"fcTL" | b"fdAT"));
    let has_picture = chunks.iter().any(|c| &c.kind == b"IDAT");
    if header_of(&chunks).is_none() || !has_picture {
        Fidelity::Impossible
    } else if animated {
        Fidelity::Loses(Loss::Animation)
    } else if !exif_resettable(file, &chunks) {
        Fidelity::Loses(Loss::Details)
    } else {
        Fidelity::Intact
    }
}

/// Whether an EXIF block that has an orientation can have it reset.
fn exif_resettable(file: &[u8], chunks: &[Chunk<'_>]) -> bool {
    match chunks.iter().find(|c| &c.kind == b"eXIf") {
        Some(exif) if ExifFacts::has_orientation(file) => {
            with_orientation(exif.data, ExifOrientation::UPRIGHT).is_ok()
        }
        Some(_) | None => true,
    }
}

fn colour_type(code: u8) -> Option<ColorType> {
    ColorType::from_u8(code)
}

fn bit_depth(code: u8) -> Option<BitDepth> {
    BitDepth::from_u8(code)
}

fn samples_of(colour: ColorType) -> usize {
    colour.samples()
}

/// Samples one per byte from rows packed at `depth` bits (1, 2 or 4), each row padded to a byte.
fn unpacked(packed: &[u8], width: usize, rows: usize, depth: usize, line: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * rows);
    let mask = (1u8 << depth) - 1;
    for row in packed.chunks(line).take(rows) {
        for x in 0..width {
            let bit = x * depth;
            out.push((row[bit / 8] >> (8 - depth - bit % 8)) & mask);
        }
    }
    out
}

/// The inverse of [`unpacked`].
fn packed(samples: &[u8], width: usize, depth: usize) -> Vec<u8> {
    let line = (width * depth).div_ceil(8);
    let mut out = Vec::with_capacity(line * samples.len().div_ceil(width.max(1)));
    for row in samples.chunks(width.max(1)) {
        let mut bytes = vec![0u8; line];
        for (x, sample) in row.iter().enumerate() {
            let bit = x * depth;
            bytes[bit / 8] |= sample << (8 - depth - bit % 8);
        }
        out.extend(bytes);
    }
    out
}

/// The picture's samples as the file stores them, rows of `unit`-byte pixels (sub-byte depths one
/// sample per byte), and the number of channels.
fn raw_grid(file: &[u8], head: &Header) -> Result<Grid, ImageError> {
    let png_err = |e: png::DecodingError| ImageError::Container {
        reason: e.to_string(),
    };
    let mut decoder = Decoder::new(Cursor::new(file));
    decoder.set_transformations(Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(png_err)?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| container("the picture is too large"))?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(png_err)?;
    let colour = colour_type(head.colour).ok_or_else(|| container("unknown colour type"))?;
    let (width, height) = (head.width as usize, head.height as usize);
    let depth = usize::from(head.depth);
    let (unit, bytes) = if depth < 8 {
        (1, unpacked(&buf, width, height, depth, frame.line_size))
    } else {
        buf.truncate(frame.buffer_size());
        (samples_of(colour) * depth / 8, buf)
    };
    Ok(Grid {
        width: head.width,
        height: head.height,
        unit,
        bytes,
    })
}

/// The compressed picture data of `grid`, as the chunks an encoder wrote for it.
fn picture_chunks(
    grid: &Grid,
    head: &Header,
    palette: Option<&[u8]>,
) -> Result<Vec<Vec<u8>>, ImageError> {
    let encode_err = |e: png::EncodingError| ImageError::Encode {
        reason: e.to_string(),
    };
    let colour = colour_type(head.colour).ok_or_else(|| container("unknown colour type"))?;
    let depth = bit_depth(head.depth).ok_or_else(|| container("unknown bit depth"))?;
    let data = if head.depth < 8 {
        packed(&grid.bytes, grid.width as usize, usize::from(head.depth))
    } else {
        grid.bytes.clone()
    };
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, grid.width, grid.height);
    encoder.set_color(colour);
    encoder.set_depth(depth);
    if let Some(palette) = palette {
        encoder.set_palette(palette.to_vec());
    }
    let mut writer = encoder.write_header().map_err(encode_err)?;
    writer.write_image_data(&data).map_err(encode_err)?;
    writer.finish().map_err(encode_err)?;
    let written = chunks(&out).ok_or_else(|| container("the encoder wrote no PNG"))?;
    Ok(written
        .iter()
        .filter(|c| &c.kind == b"IDAT")
        .map(|c| c.raw.to_vec())
        .collect())
}

/// `file` with `placing` applied, everything else as it was.
pub(super) fn rewritten(file: &[u8], placing: Placing) -> Result<Vec<u8>, ImageError> {
    let chunks = chunks(file).ok_or_else(|| container("not a PNG"))?;
    let head = header_of(&chunks).ok_or_else(|| container("no PNG header"))?;
    let total = placing.over(ExifFacts::read(file).orientation);
    let grid = raw_grid(file, &head)?.placed(total);
    let palette = chunks.iter().find(|c| &c.kind == b"PLTE").map(|c| c.data);
    let mut pictured = picture_chunks(&grid, &head, palette)?.into_iter();

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&grid.width.to_be_bytes());
    ihdr.extend_from_slice(&grid.height.to_be_bytes());
    ihdr.extend_from_slice(&[head.depth, head.colour, 0, 0, 0]);
    let mut out = SIGNATURE.to_vec();
    out.extend(chunk_bytes(b"IHDR", &ihdr));
    let mut idat_done = false;
    for chunk in &chunks {
        match &chunk.kind {
            b"IHDR" | b"IEND" | b"acTL" | b"fcTL" | b"fdAT" => {}
            b"IDAT" => {
                if !idat_done {
                    idat_done = true;
                    pictured.by_ref().for_each(|raw| out.extend(raw));
                }
            }
            b"pHYs" if swaps_axes(total) && chunk.data.len() == 9 => {
                let swapped = [&chunk.data[4..8], &chunk.data[0..4], &chunk.data[8..9]].concat();
                out.extend(chunk_bytes(b"pHYs", &swapped));
            }
            b"eXIf" if ExifFacts::has_orientation(file) => {
                if let Ok(upright) = with_orientation(chunk.data, ExifOrientation::UPRIGHT) {
                    out.extend(chunk_bytes(b"eXIf", &upright));
                }
            }
            _ => out.extend_from_slice(chunk.raw),
        }
    }
    out.extend(chunk_bytes(b"IEND", &[]));
    Ok(out)
}
