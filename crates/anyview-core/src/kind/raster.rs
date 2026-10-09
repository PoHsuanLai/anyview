//! Raster image formats the viewer opens.

use super::family::Family;
use ds_core::word::Word;

/// A raster image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum RasterFormat {
    /// PNG, including animated PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// GIF, animated or not.
    Gif,
    /// WebP, animated or not.
    Webp,
    /// Windows bitmap.
    Bmp,
    /// TIFF.
    Tiff,
    /// Windows icon and cursor.
    Ico,
    /// Targa.
    Tga,
    /// Quite OK Image.
    Qoi,
    /// AVIF.
    Avif,
    /// JPEG XL.
    Jxl,
    /// HEIC and HEIF.
    Heic,
    /// Photoshop, shown flattened.
    Psd,
    /// Apple icon.
    Icns,
    /// OpenEXR, tone mapped.
    Exr,
    /// Radiance HDR, tone mapped.
    Hdr,
    /// A camera raw file, shown from its embedded preview.
    Raw,
}

impl Family for RasterFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            RasterFormat::Png => &["png", "apng"],
            RasterFormat::Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            RasterFormat::Gif => &["gif"],
            RasterFormat::Webp => &["webp"],
            RasterFormat::Bmp => &["bmp", "dib"],
            RasterFormat::Tiff => &["tif", "tiff"],
            RasterFormat::Ico => &["ico", "cur"],
            RasterFormat::Tga => &["tga"],
            RasterFormat::Qoi => &["qoi"],
            RasterFormat::Avif => &["avif"],
            RasterFormat::Jxl => &["jxl"],
            RasterFormat::Heic => &["heic", "heif", "hif"],
            RasterFormat::Psd => &["psd"],
            RasterFormat::Icns => &["icns"],
            RasterFormat::Exr => &["exr"],
            RasterFormat::Hdr => &["hdr"],
            RasterFormat::Raw => &["cr2", "cr3", "nef", "arw", "dng", "orf", "rw2"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            RasterFormat::Png => "image/png",
            RasterFormat::Jpeg => "image/jpeg",
            RasterFormat::Gif => "image/gif",
            RasterFormat::Webp => "image/webp",
            RasterFormat::Bmp => "image/bmp",
            RasterFormat::Tiff => "image/tiff",
            RasterFormat::Ico => "image/vnd.microsoft.icon",
            RasterFormat::Tga => "image/x-tga",
            RasterFormat::Qoi => "image/x-qoi",
            RasterFormat::Avif => "image/avif",
            RasterFormat::Jxl => "image/jxl",
            RasterFormat::Heic => "image/heic",
            RasterFormat::Psd => "image/vnd.adobe.photoshop",
            RasterFormat::Icns => "image/x-icns",
            RasterFormat::Exr => "image/x-exr",
            RasterFormat::Hdr => "image/vnd.radiance",
            RasterFormat::Raw => "image/x-dcraw",
        }
    }
}
