//! The encoders the `image` crate provides: PNG, JPEG, lossless WebP, TIFF and BMP.

use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::Quality;
use image::codecs::bmp::BmpEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::tiff::TiffEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::io::Cursor;

fn encode_error(error: image::ImageError) -> ImageError {
    ImageError::Encode {
        reason: error.to_string(),
    }
}

fn dimensions(picture: &Rgba8) -> (u32, u32) {
    let size = picture.size();
    (size.width.0, size.height.0)
}

pub(super) fn png(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let (width, height) = dimensions(picture);
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(picture.bytes(), width, height, ExtendedColorType::Rgba8)
        .map_err(encode_error)?;
    Ok(out)
}

/// The pixels over white, three bytes each: JPEG has no alpha.
fn flattened(picture: &Rgba8) -> Vec<u8> {
    picture
        .bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| {
            let alpha = u32::from(pixel[3]);
            [pixel[0], pixel[1], pixel[2]]
                .map(|c| ((u32::from(c) * alpha + 255 * (255 - alpha) + 127) / 255) as u8) // at most 255
        })
        .collect()
}

pub(super) fn jpeg(picture: &Rgba8, quality: Quality) -> Result<Vec<u8>, ImageError> {
    let (width, height) = dimensions(picture);
    let percent = u8::try_from(quality.percent().0).unwrap_or(100); // a quality is 1 to 100
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, percent)
        .write_image(&flattened(picture), width, height, ExtendedColorType::Rgb8)
        .map_err(encode_error)?;
    Ok(out)
}

pub(super) fn webp(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let (width, height) = dimensions(picture);
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .write_image(picture.bytes(), width, height, ExtendedColorType::Rgba8)
        .map_err(encode_error)?;
    Ok(out)
}

pub(super) fn tiff(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let (width, height) = dimensions(picture);
    let mut out = Cursor::new(Vec::new());
    TiffEncoder::new(&mut out)
        .write_image(picture.bytes(), width, height, ExtendedColorType::Rgba8)
        .map_err(encode_error)?;
    Ok(out.into_inner())
}

pub(super) fn bmp(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let (width, height) = dimensions(picture);
    let mut out = Vec::new();
    BmpEncoder::new(&mut out)
        .write_image(picture.bytes(), width, height, ExtendedColorType::Rgba8)
        .map_err(encode_error)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{PixelLen, PixelSize};

    #[test]
    fn jpeg_composites_alpha_onto_white() {
        // name, pixel, flattened
        const CASES: &[(&str, [u8; 4], [u8; 3])] = &[
            ("opaque", [10, 20, 30, 255], [10, 20, 30]),
            ("transparent is white", [10, 20, 30, 0], [255, 255, 255]),
            ("half black is grey", [0, 0, 0, 128], [127, 127, 127]),
        ];
        for (name, pixel, want) in CASES {
            let size = PixelSize {
                width: PixelLen(1),
                height: PixelLen(1),
            };
            let picture = Rgba8::new(size, pixel.to_vec()).unwrap();
            assert_eq!(flattened(&picture), want, "{name}");
        }
    }
}
