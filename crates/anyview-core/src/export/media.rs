//! What a video can be exported as.

use super::choice::ExportChoice;
use super::extension::ExportExtension;
use super::target::RasterTarget;
use ds_core::word::Word;

/// An export of a video or audio recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaExport {
    /// The frame on screen, at the video's full resolution.
    CurrentFrame(RasterTarget),
}

/// The entries of the media export pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum MediaExportKind {
    /// The current frame as a PNG.
    FramePng,
    /// The current frame as a JPEG.
    FrameJpeg,
    /// The current frame as lossless WebP.
    FrameWebp,
    /// The current frame as an AVIF.
    FrameAvif,
    /// The current frame as a TIFF.
    FrameTiff,
}

impl ExportChoice for MediaExport {
    type Kind = MediaExportKind;

    fn kind(&self) -> MediaExportKind {
        match self {
            MediaExport::CurrentFrame(RasterTarget::Png) => MediaExportKind::FramePng,
            MediaExport::CurrentFrame(RasterTarget::Jpeg(_)) => MediaExportKind::FrameJpeg,
            MediaExport::CurrentFrame(RasterTarget::Webp) => MediaExportKind::FrameWebp,
            MediaExport::CurrentFrame(RasterTarget::Avif(_)) => MediaExportKind::FrameAvif,
            MediaExport::CurrentFrame(RasterTarget::Tiff) => MediaExportKind::FrameTiff,
        }
    }

    fn default_for(kind: MediaExportKind) -> Self {
        match kind {
            MediaExportKind::FramePng => MediaExport::CurrentFrame(RasterTarget::Png),
            MediaExportKind::FrameJpeg => MediaExport::CurrentFrame(RasterTarget::default_jpeg()),
            MediaExportKind::FrameWebp => MediaExport::CurrentFrame(RasterTarget::Webp),
            MediaExportKind::FrameAvif => MediaExport::CurrentFrame(RasterTarget::default_avif()),
            MediaExportKind::FrameTiff => MediaExport::CurrentFrame(RasterTarget::Tiff),
        }
    }

    fn extension(&self) -> ExportExtension {
        match self {
            MediaExport::CurrentFrame(target) => target.extension(),
        }
    }
}
