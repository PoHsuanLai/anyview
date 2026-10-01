//! The pixel buffers every part of the crate passes around: straight and premultiplied RGBA8.

use crate::error::ImageError;
use anyview_core::{PixelLen, PixelSize};
use image::RgbaImage;
use std::fmt;

/// A picture of 8-bit sRGB pixels with **straight** (non-premultiplied) alpha, four bytes per
/// pixel in `R G B A` order, rows top to bottom with no padding.
///
/// Straight alpha is what every decoder and encoder here speaks, and what a CPU-side consumer
/// wants. A GPU compositor blends with premultiplied alpha: [`Rgba8::premultiplied`] is the one
/// conversion, and [`PremultipliedRgba8`] is a distinct type so the two cannot be mixed up.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgba8 {
    size: PixelSize,
    bytes: Vec<u8>,
}

/// The same picture with each colour channel already multiplied by its alpha.
#[derive(Clone, PartialEq, Eq)]
pub struct PremultipliedRgba8 {
    size: PixelSize,
    bytes: Vec<u8>,
}

/// Bytes a picture of `size` holds, or `None` when that overflows `usize`.
fn byte_len(size: PixelSize) -> Option<usize> {
    let pixels = usize::try_from(size.area().0).ok()?;
    pixels.checked_mul(4)
}

fn checked(size: PixelSize, bytes: &[u8]) -> Result<(), ImageError> {
    if byte_len(size) == Some(bytes.len()) {
        Ok(())
    } else {
        Err(ImageError::PixelsMismatch {
            size,
            len: bytes.len(),
        })
    }
}

impl Rgba8 {
    /// A picture of `size` made of `bytes`, or why they do not fit.
    pub fn new(size: PixelSize, bytes: Vec<u8>) -> Result<Self, ImageError> {
        checked(size, &bytes)?;
        Ok(Rgba8 { size, bytes })
    }

    /// The width and height in pixels.
    pub fn size(&self) -> PixelSize {
        self.size
    }

    /// The pixels, `R G B A` per pixel.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The pixels, given up.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// The picture with alpha multiplied into the colour, rounded to nearest: what a GPU blends.
    pub fn premultiplied(&self) -> PremultipliedRgba8 {
        let mut bytes = self.bytes.clone();
        for pixel in bytes.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8; // at most 255
            }
        }
        PremultipliedRgba8 {
            size: self.size,
            bytes,
        }
    }

    pub(crate) fn from_image(image: RgbaImage) -> Self {
        let size = PixelSize {
            width: PixelLen(image.width()),
            height: PixelLen(image.height()),
        };
        Rgba8 {
            size,
            bytes: image.into_raw(),
        }
    }

    pub(crate) fn to_image(&self) -> Option<RgbaImage> {
        RgbaImage::from_raw(self.size.width.0, self.size.height.0, self.bytes.clone())
    }
}

impl PremultipliedRgba8 {
    pub(crate) fn from_image(image: RgbaImage) -> Self {
        let size = PixelSize {
            width: PixelLen(image.width()),
            height: PixelLen(image.height()),
        };
        PremultipliedRgba8 {
            size,
            bytes: image.into_raw(),
        }
    }

    pub(crate) fn to_image(&self) -> Option<RgbaImage> {
        RgbaImage::from_raw(self.size.width.0, self.size.height.0, self.bytes.clone())
    }

    /// A premultiplied picture of `size` made of `bytes`, or why they do not fit.
    pub fn new(size: PixelSize, bytes: Vec<u8>) -> Result<Self, ImageError> {
        checked(size, &bytes)?;
        Ok(PremultipliedRgba8 { size, bytes })
    }

    /// The width and height in pixels.
    pub fn size(&self) -> PixelSize {
        self.size
    }

    /// The pixels, `R G B A` per pixel, colour already multiplied by alpha.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The picture with alpha divided back out. Fully transparent pixels become transparent
    /// black, since their colour is gone.
    pub fn unpremultiplied(&self) -> Rgba8 {
        let mut bytes = self.bytes.clone();
        for pixel in bytes.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = match alpha {
                    0 => 0,
                    _ => ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8, // clamped
                };
            }
        }
        Rgba8 {
            size: self.size,
            bytes,
        }
    }
}

impl fmt::Debug for Rgba8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgba8({}x{})", self.size.width.0, self.size.height.0)
    }
}

impl fmt::Debug for PremultipliedRgba8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PremultipliedRgba8({}x{})",
            self.size.width.0, self.size.height.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> PixelSize {
        PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        }
    }

    #[test]
    fn a_buffer_must_hold_four_bytes_per_pixel() {
        // name, width, height, bytes, fits
        const CASES: &[(&str, u32, u32, usize, bool)] = &[
            ("exact", 2, 3, 24, true),
            ("empty", 0, 5, 0, true),
            ("short", 2, 3, 23, false),
            ("long", 2, 3, 25, false),
        ];
        for (name, w, h, len, fits) in CASES {
            let made = Rgba8::new(size(*w, *h), vec![0; *len]);
            assert_eq!(made.is_ok(), *fits, "{name}");
        }
    }

    #[test]
    fn premultiplying_rounds_and_unpremultiplying_recovers_opaque_and_transparent() {
        // name, straight pixel, premultiplied pixel
        const CASES: &[(&str, [u8; 4], [u8; 4])] = &[
            ("opaque", [200, 100, 50, 255], [200, 100, 50, 255]),
            ("transparent drops colour", [200, 100, 50, 0], [0, 0, 0, 0]),
            ("half", [255, 128, 0, 128], [128, 64, 0, 128]),
            ("quarter rounds", [255, 255, 255, 64], [64, 64, 64, 64]),
        ];
        for (name, straight, premultiplied) in CASES {
            let picture = Rgba8::new(size(1, 1), straight.to_vec()).unwrap();
            assert_eq!(
                picture.premultiplied().bytes(),
                premultiplied,
                "{name} premultiplied"
            );
        }
        let opaque = Rgba8::new(size(1, 1), vec![200, 100, 50, 255]).unwrap();
        assert_eq!(opaque.premultiplied().unpremultiplied(), opaque);
        let gone = PremultipliedRgba8::new(size(1, 1), vec![9, 9, 9, 0]).unwrap();
        assert_eq!(gone.unpremultiplied().bytes(), [0, 0, 0, 0]);
    }

    #[test]
    fn unpremultiplying_never_exceeds_the_channel_range() {
        // A premultiplied value above its alpha is malformed input from a GPU readback.
        let bad = PremultipliedRgba8::new(size(1, 1), vec![250, 0, 0, 100]).unwrap();
        assert_eq!(bad.unpremultiplied().bytes(), [255, 0, 0, 100]);
    }
}
