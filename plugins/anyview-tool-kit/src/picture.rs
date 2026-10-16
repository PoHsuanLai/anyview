//! The picture a tool wrote, read back as straight RGBA8 and scaled to what the host asked for.

use crate::error::ToolError;
use anyview_plugin_common::{Fit, Picture, fitted};
use image::{ImageReader, Limits, imageops::FilterType};
use std::path::Path;

/// The most memory decoding a picture may allocate: a 16384 x 16384 RGBA picture and some more.
const MAX_ALLOC: u64 = 2 << 30;

/// Reads the picture file at `path` (PNG, TIFF, PPM or JPEG, whichever its bytes say) and scales
/// it down to `fit`. The tool has already turned it upright.
pub fn load_picture(path: &Path, fit: Fit) -> Result<Picture, ToolError> {
    let mut reader = ImageReader::open(path)
        .map_err(|error| ToolError::io("open the tool's picture", &error))?
        .with_guessed_format()
        .map_err(|error| ToolError::io("read the tool's picture", &error))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => ToolError::TooLarge(error.to_string()),
        image::ImageError::Decoding(_)
        | image::ImageError::Encoding(_)
        | image::ImageError::Parameter(_)
        | image::ImageError::Unsupported(_)
        | image::ImageError::IoError(_) => ToolError::Corrupt(error.to_string()),
    })?;
    let rgba = decoded.into_rgba8();
    let (width, height) = fitted(rgba.dimensions(), fit);
    let rgba = if (width, height) == rgba.dimensions() {
        rgba
    } else {
        image::imageops::resize(&rgba, width, height, FilterType::Triangle)
    };
    Ok(Picture {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

/// `picture` scaled down to `fit`; as it is when it already fits.
pub fn scaled(picture: Picture, fit: Fit) -> Picture {
    let (width, height) = fitted((picture.width, picture.height), fit);
    if (width, height) == (picture.width, picture.height) {
        return picture;
    }
    let Some(image) = image::RgbaImage::from_raw(picture.width, picture.height, picture.rgba)
    else {
        // A picture this crate made always has the bytes its size says.
        return Picture {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
    };
    let out = image::imageops::resize(&image, width, height, FilterType::Triangle);
    Picture {
        width,
        height,
        rgba: out.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_tiff_and_ppm_are_read_and_a_big_one_is_scaled() {
        let dir = tempfile::tempdir().unwrap();
        let image = image::RgbImage::from_fn(40, 20, |x, _| image::Rgb([x as u8 * 6, 9, 200]));
        for name in ["a.png", "a.tiff", "a.ppm"] {
            let path = dir.path().join(name);
            image.save(&path).unwrap();
            let whole = load_picture(&path, Fit::Edge(100)).unwrap();
            assert_eq!((whole.width, whole.height), (40, 20), "{name}");
            assert_eq!(whole.rgba.len(), 40 * 20 * 4, "{name}");
            assert_eq!(&whole.rgba[..4], &[0, 9, 200, 255], "{name}");
            let small = load_picture(&path, Fit::Edge(10)).unwrap();
            assert_eq!((small.width, small.height), (10, 5), "{name}");
        }
        let junk = dir.path().join("junk.png");
        std::fs::write(&junk, b"not a picture").unwrap();
        assert!(matches!(
            load_picture(&junk, Fit::Edge(10)),
            Err(ToolError::Corrupt(_))
        ));
    }
}
