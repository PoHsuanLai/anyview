//! A lossless WebP turned or mirrored and written lossless again, with its colour profile, EXIF
//! (orientation reset, the pixels carry it now), XMP and any other chunk kept. A lossy WebP is
//! written lossless, since no pure-Rust encoder writes lossy: the picture loses nothing more,
//! and the file grows. An animated one keeps its first frame.

use super::{Fidelity, Loss};
use crate::error::ImageError;
use crate::exif::{ExifFacts, with_orientation};
use crate::orientation::ExifOrientation;
use crate::pixels::Rgba8;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};

const VP8X_ICC: u8 = 0x20;
const VP8X_ALPHA: u8 = 0x10;
const VP8X_EXIF: u8 = 0x08;
const VP8X_XMP: u8 = 0x04;

/// One RIFF chunk: its four-character code and its data without the padding byte.
struct Chunk<'a> {
    code: [u8; 4],
    data: &'a [u8],
}

fn chunks(file: &[u8]) -> Option<Vec<Chunk<'_>>> {
    if file.get(..4)? != b"RIFF" || file.get(8..12)? != b"WEBP" {
        return None;
    }
    let mut at = 12;
    let mut found = Vec::new();
    while at + 8 <= file.len() {
        let code: [u8; 4] = file[at..at + 4].try_into().ok()?;
        let size =
            usize::try_from(u32::from_le_bytes(file[at + 4..at + 8].try_into().ok()?)).ok()?;
        let data = file.get(at + 8..at + 8 + size)?;
        found.push(Chunk { code, data });
        at += 8 + size + (size & 1);
    }
    Some(found)
}

/// What turning the file costs. A lossy picture is written lossless, which is no further loss
/// of picture, and an animation keeps one frame.
pub(super) fn fidelity(file: &[u8]) -> Fidelity {
    let Some(chunks) = chunks(file) else {
        return Fidelity::Impossible;
    };
    let has = |code: &[u8; 4]| chunks.iter().any(|c| &c.code == code);
    if !(has(b"VP8L") || has(b"VP8 ") || has(b"ANMF")) {
        return Fidelity::Impossible;
    }
    let exif_ok = match chunks.iter().find(|c| &c.code == b"EXIF") {
        Some(exif) if ExifFacts::has_orientation(file) => {
            with_orientation(exif.data, ExifOrientation::UPRIGHT).is_ok()
        }
        Some(_) | None => true,
    };
    if has(b"ANMF") {
        Fidelity::Loses(Loss::Animation)
    } else if exif_ok {
        Fidelity::Intact
    } else {
        Fidelity::Loses(Loss::Details)
    }
}

fn chunk(code: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = code.to_vec();
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_le_bytes()); // a chunk is under 4 GiB
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// The lossless picture data of `picture`: the payload of the `VP8L` chunk an encoder wrote.
fn picture_data(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let size = picture.size();
    let opaque = picture
        .bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|px| px[3] == 255);
    let (bytes, colour) = if opaque {
        let rgb = picture
            .bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| [px[0], px[1], px[2]])
            .collect();
        (rgb, ExtendedColorType::Rgb8)
    } else {
        (picture.bytes().to_vec(), ExtendedColorType::Rgba8)
    };
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .write_image(&bytes, size.width.0, size.height.0, colour)
        .map_err(|e| ImageError::Encode {
            reason: e.to_string(),
        })?;
    chunks(&out)
        .and_then(|found| found.into_iter().find(|c| &c.code == b"VP8L"))
        .map(|c| c.data.to_vec())
        .ok_or_else(|| ImageError::Encode {
            reason: "the encoder wrote no lossless picture".to_owned(),
        })
}

/// `moved` (the picture of `file`, already turned) written as lossless WebP with the chunks of
/// `file` that are not the picture.
pub(super) fn rewritten(file: &[u8], moved: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let original = chunks(file).ok_or_else(|| ImageError::Container {
        reason: "not a WebP".to_owned(),
    })?;
    let picture = picture_data(moved)?;
    let mut flags = if moved
        .bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|px| px[3] == 255)
    {
        0u8
    } else {
        VP8X_ALPHA
    };
    let mut before = Vec::new();
    let mut after = Vec::new();
    for c in &original {
        match &c.code {
            b"ICCP" => {
                flags |= VP8X_ICC;
                before.extend(chunk(b"ICCP", c.data));
            }
            b"EXIF" => {
                flags |= VP8X_EXIF;
                let data = if ExifFacts::has_orientation(file) {
                    with_orientation(c.data, ExifOrientation::UPRIGHT).ok()
                } else {
                    Some(c.data.to_vec())
                };
                if let Some(data) = data {
                    after.extend(chunk(b"EXIF", &data));
                }
            }
            b"XMP " => {
                flags |= VP8X_XMP;
                after.extend(chunk(b"XMP ", c.data));
            }
            b"VP8X" | b"VP8L" | b"VP8 " | b"ALPH" | b"ANIM" | b"ANMF" => {}
            code => after.extend(chunk(code, c.data)),
        }
    }
    let mut body = b"WEBP".to_vec();
    if flags != 0 || !before.is_empty() || !after.is_empty() {
        let size = moved.size();
        let mut vp8x = vec![flags, 0, 0, 0];
        vp8x.extend_from_slice(&(size.width.0 - 1).to_le_bytes()[..3]);
        vp8x.extend_from_slice(&(size.height.0 - 1).to_le_bytes()[..3]);
        body.extend(chunk(b"VP8X", &vp8x));
    }
    body.extend(before);
    body.extend(chunk(b"VP8L", &picture));
    body.extend(after);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_le_bytes()); // a file is under 4 GiB
    out.extend(body);
    Ok(out)
}
