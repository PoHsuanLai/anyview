//! What a raster image can be exported as.

use super::choice::ExportChoice;
use super::extension::ExportExtension;
use super::target::{RasterTarget, Resize};
use ds_core::word::Word;

/// An export of a raster image: re-encoded, or placed on a PDF page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RasterExport {
    /// Encode the pixels, resized first.
    Image(RasterTarget, Resize),
    /// One PDF page holding the image.
    Pdf,
}

/// The entries of the raster export pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum RasterExportKind {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// Lossless WebP.
    Webp,
    /// AVIF.
    Avif,
    /// TIFF.
    Tiff,
    /// PDF.
    Pdf,
}

impl ExportChoice for RasterExport {
    type Kind = RasterExportKind;

    fn kind(&self) -> RasterExportKind {
        match self {
            RasterExport::Image(target, _) => match target {
                RasterTarget::Png => RasterExportKind::Png,
                RasterTarget::Jpeg(_) => RasterExportKind::Jpeg,
                RasterTarget::Webp => RasterExportKind::Webp,
                RasterTarget::Avif(_) => RasterExportKind::Avif,
                RasterTarget::Tiff => RasterExportKind::Tiff,
            },
            RasterExport::Pdf => RasterExportKind::Pdf,
        }
    }

    fn default_for(kind: RasterExportKind) -> Self {
        let image = |target| RasterExport::Image(target, Resize::Original);
        match kind {
            RasterExportKind::Png => image(RasterTarget::Png),
            RasterExportKind::Jpeg => image(RasterTarget::default_jpeg()),
            RasterExportKind::Webp => image(RasterTarget::Webp),
            RasterExportKind::Avif => image(RasterTarget::default_avif()),
            RasterExportKind::Tiff => image(RasterTarget::Tiff),
            RasterExportKind::Pdf => RasterExport::Pdf,
        }
    }

    fn extension(&self) -> ExportExtension {
        match self {
            RasterExport::Image(target, _) => target.extension(),
            RasterExport::Pdf => ExportExtension::Pdf,
        }
    }
}
