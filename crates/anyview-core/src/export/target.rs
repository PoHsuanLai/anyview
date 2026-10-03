//! The pieces exports share: how a raster image is encoded and how it is resized first.

use super::extension::ExportExtension;
use crate::units::{Percent, Permille, PixelLen, Quality};

/// A raster encoding with the options it has. WebP is lossless only (the pure-Rust encoder has no
/// lossy mode), so it carries none; there is no HEIC or JPEG XL target because no pure-Rust
/// encoder exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RasterTarget {
    /// PNG.
    Png,
    /// JPEG at a quality.
    Jpeg(Quality),
    /// Lossless WebP.
    Webp,
    /// AVIF at a quality.
    Avif(Quality),
    /// TIFF.
    Tiff,
}

impl RasterTarget {
    /// The quality a new JPEG export starts at.
    pub fn default_jpeg() -> RasterTarget {
        RasterTarget::Jpeg(Quality::clamped(Percent(90)))
    }

    /// The quality a new AVIF export starts at; AVIF stays sharp at lower numbers than JPEG.
    pub fn default_avif() -> RasterTarget {
        RasterTarget::Avif(Quality::clamped(Percent(65)))
    }

    /// The extension of the file this encoding writes.
    pub fn extension(self) -> ExportExtension {
        match self {
            RasterTarget::Png => ExportExtension::Png,
            RasterTarget::Jpeg(_) => ExportExtension::Jpg,
            RasterTarget::Webp => ExportExtension::Webp,
            RasterTarget::Avif(_) => ExportExtension::Avif,
            RasterTarget::Tiff => ExportExtension::Tiff,
        }
    }
}

/// How an image is resized before it is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Resize {
    /// Keep the pixels as they are.
    #[default]
    Original,
    /// Scale both sides by a proportion, 1000 permille being the original.
    Scaled(Permille),
    /// Scale so the longer side has this many pixels.
    LongEdge(PixelLen),
}
