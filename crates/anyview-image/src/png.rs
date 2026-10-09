//! PNG writing, outside the `encode` feature: the `image` crate's PNG codec is compiled in to read
//! PNGs anyway, so a caller that only peeks (a launcher showing an audio file's cover) can write one
//! without linking the other encoders.

use crate::error::ImageError;
use crate::pixels::Rgba8;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

/// `picture` as PNG bytes, straight alpha kept.
pub fn encode_png(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    let size = picture.size();
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(
            picture.bytes(),
            size.width.0,
            size.height.0,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| ImageError::Encode {
            reason: error.to_string(),
        })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::encode_png;
    use crate::pixels::Rgba8;
    use anyview_core::{PixelLen, PixelSize};

    #[test]
    fn a_picture_comes_back_from_its_png_pixel_for_pixel() {
        let bytes = vec![255, 0, 0, 255, 0, 0, 255, 128];
        let size = PixelSize {
            width: PixelLen(2),
            height: PixelLen(1),
        };
        let picture = Rgba8::new(size, bytes.clone()).unwrap();
        let png = encode_png(&picture).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        let back = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!((back.width(), back.height()), (2, 1));
        assert_eq!(back.into_raw(), bytes);
    }
}
