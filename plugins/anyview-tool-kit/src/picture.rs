//! The picture a tool wrote, read back as straight RGBA8 and scaled to what the host asked for.

use crate::error::ToolError;
use image::{ImageReader, Limits, imageops::FilterType};
use std::path::Path;

/// The most memory decoding a picture may allocate: a 16384 x 16384 RGBA picture and some more.
const MAX_ALLOC: u64 = 2 << 30;

/// What the picture may measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// No side longer than this many pixels.
    Edge(u32),
    /// No more than this many pixels in all.
    Area(u64),
}

/// A picture as straight RGBA8 rows from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// The size within `fit` that keeps the shape of `size` and never enlarges it.
pub fn fitted(size: (u32, u32), fit: Fit) -> (u32, u32) {
    let (w, h) = (u64::from(size.0), u64::from(size.1));
    let scaled = |scale: f64| {
        let side = |n: u64| ((n as f64 * scale).floor() as u64).max(1);
        (side(w), side(h))
    };
    let (nw, nh) = match fit {
        Fit::Edge(edge) => {
            let longer = w.max(h);
            if longer <= u64::from(edge) {
                (w, h)
            } else {
                scaled(f64::from(edge) / longer as f64)
            }
        }
        Fit::Area(area) => {
            if w * h <= area {
                (w, h)
            } else {
                let (mut nw, mut nh) = scaled((area as f64 / (w * h) as f64).sqrt());
                while nw * nh > area && (nw > 1 || nh > 1) {
                    if nw > 1 {
                        nw -= 1;
                    } else {
                        nh -= 1;
                    }
                }
                (nw, nh)
            }
        }
    };
    (
        u32::try_from(nw).unwrap_or(u32::MAX),
        u32::try_from(nh).unwrap_or(u32::MAX),
    )
}

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
    fn a_picture_is_fitted_without_being_enlarged() {
        type Size = (u32, u32);
        const CASES: &[(&str, Size, Fit, Size)] = &[
            ("small stays", (64, 48), Fit::Edge(256), (64, 48)),
            ("wide", (1920, 1080), Fit::Edge(256), (256, 144)),
            ("tall", (1080, 1920), Fit::Edge(256), (144, 256)),
            ("area allows", (64, 48), Fit::Area(10_000), (64, 48)),
            (
                "area shrinks",
                (6000, 4000),
                Fit::Area(1_000_000),
                (1224, 816),
            ),
            ("a sliver", (10_000, 1), Fit::Area(100), (100, 1)),
        ];
        for (name, size, fit, want) in CASES {
            let got = fitted(*size, *fit);
            assert_eq!(got, *want, "{name}");
        }
    }

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
