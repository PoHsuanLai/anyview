//! OpenEXR and Radiance HDR: scene-linear floating point pictures shown as 8-bit sRGB.
//!
//! **Tone mapping.** A channel `c` (linear, at least 0) goes through the extended Reinhard curve
//! `c * (1 + c / w^2) / (1 + c)`, where `w` is the largest value in the picture, never below 1. A
//! picture whose values all lie in 0..=1 therefore keeps its tones, and one with highlights above
//! 1 is compressed so the brightest value lands on white. The result is encoded with the sRGB
//! transfer curve. Alpha is clamped to 0..=1 and scaled, not curved.

use super::ceiling::Ceiling;
use super::colour::ColourInfo;
use super::stills::{decode_error, header, limits};
use crate::error::ImageError;
use crate::pixels::Rgba8;
use image::{ImageFormat, ImageReader};
use std::io::Cursor;

/// What a picture costs per pixel beyond the codec's own buffer at its peak: the f32 RGBA copy the
/// tone mapping reads and the RGBA8 it writes.
pub(crate) const COPIES: u64 = 16 + 4;

/// The picture of an EXR or HDR file, tone mapped to straight RGBA8.
pub(crate) fn decode(
    bytes: &[u8],
    format: ImageFormat,
    ceiling: Ceiling,
) -> Result<(Rgba8, ColourInfo), ImageError> {
    let (size, per_pixel) = header(bytes, format)?.cost(COPIES);
    ceiling.admit(size, per_pixel)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits(ceiling));
    let decoded = reader.decode().map_err(|e| decode_error(e, Some(size)))?;
    let colour = ColourInfo::of_color_type(decoded.color());
    let linear = decoded.into_rgba32f();
    let white = white_point(linear.as_raw());
    let bytes = tone_mapped(linear.as_raw(), white);
    Ok((Rgba8::new(size, bytes)?, colour))
}

/// The largest colour value in `samples` (RGBA, four to a pixel), at least 1.
fn white_point(samples: &[f32]) -> f32 {
    samples
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| pixel[..3].iter().copied())
        .filter(|value| value.is_finite())
        .fold(1.0_f32, f32::max)
}

fn tone_mapped(samples: &[f32], white: f32) -> Vec<u8> {
    samples
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| {
            [
                encoded(reinhard(pixel[0], white)),
                encoded(reinhard(pixel[1], white)),
                encoded(reinhard(pixel[2], white)),
                byte(pixel[3]),
            ]
        })
        .collect()
}

/// Extended Reinhard: maps `0..=white` onto `0..=1`.
pub(crate) fn reinhard(value: f32, white: f32) -> f32 {
    let value = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };
    value * (1.0 + value / (white * white)) / (1.0 + value)
}

/// A linear value in 0..=1 as an sRGB byte.
pub(crate) fn encoded(linear: f32) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let curved = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    byte(curved)
}

fn byte(value: f32) -> u8 {
    let value = if value.is_finite() { value } else { 0.0 };
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8 // clamped to 0..=255
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tones_follow_the_curve() {
        // name, linear value, white point, byte
        const CASES: &[(&str, f32, f32, u8)] = &[
            ("black stays black", 0.0, 1.0, 0),
            ("white in range stays white", 1.0, 1.0, 255),
            ("the white point lands on white", 8.0, 8.0, 255),
            ("a negative is black", -2.0, 1.0, 0),
            ("not a number is black", f32::NAN, 1.0, 0),
            ("in range mid grey is sRGB 188", 0.5, 1.0, 188),
        ];
        for (name, value, white, want) in CASES {
            assert_eq!(encoded(reinhard(*value, *white)), *want, "{name}");
        }
    }
}
