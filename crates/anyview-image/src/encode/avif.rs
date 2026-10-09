//! AVIF through `ravif`, a pure-Rust encoder. Slow: it is the one encoder that needs a worker and
//! a progress indicator for a large picture.

use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::Quality;
use ravif::{Encoder, Img, RGBA8};

/// The AV1 encoder's speed, 1 (slowest, smallest) to 10. Six is the encoder's own balanced default.
const SPEED: u8 = 6;

/// `picture` as AVIF at `quality`, with `exif` (a raw TIFF block) and the ICC `profile` embedded
/// when given.
pub(super) fn encode(
    picture: &Rgba8,
    quality: Quality,
    exif: Option<&[u8]>,
    profile: Option<&[u8]>,
) -> Result<Vec<u8>, ImageError> {
    let size = picture.size();
    let pixels: Vec<RGBA8> = picture
        .bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|[r, g, b, a]| RGBA8::new(*r, *g, *b, *a))
        .collect();
    let image = Img::new(
        pixels.as_slice(),
        size.width.0 as usize, // u32 into usize
        size.height.0 as usize,
    );
    let mut encoder = Encoder::new()
        .with_quality(f32::from(quality.percent().0))
        .with_speed(SPEED);
    if let Some(exif) = exif {
        encoder = encoder.with_exif(exif.to_vec());
    }
    let file = encoder
        .encode_rgba(image)
        .map(|encoded| encoded.avif_file)
        .map_err(|e| ImageError::Encode {
            reason: e.to_string(),
        })?;
    match profile {
        Some(profile) => super::icc::with_profile(file, profile),
        None => Ok(file),
    }
}
