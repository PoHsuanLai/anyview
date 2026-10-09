//! What a video or an audio file can be exported as.

use super::choice::ExportChoice;
use super::extension::ExportExtension;
use super::target::{AudioTarget, RasterTarget};
use crate::units::TimeRange;
use ds_core::word::Word;

/// An export of a video or audio recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaExport {
    /// The frame on screen, at the video's full resolution.
    CurrentFrame(RasterTarget),
    /// A part of the recording, cut at a keyframe with no re-encode, written as a copy.
    Trim(TimeRange),
    /// The audio track of the recording, copied or re-encoded.
    AudioOnly(AudioTarget),
}

/// The rows of the media export dialog's format list.
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
    /// A part of the recording, cut without re-encoding.
    Trim,
    /// The audio track as it is.
    ExtractAudio,
    /// The audio as AAC in an M4A file.
    ToM4a,
    /// The audio as MP3.
    ToMp3,
    /// The audio as FLAC.
    ToFlac,
    /// The audio as WAV.
    ToWav,
    /// The audio as Opus.
    ToOpus,
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
            MediaExport::Trim(_) => MediaExportKind::Trim,
            MediaExport::AudioOnly(AudioTarget::Copy) => MediaExportKind::ExtractAudio,
            MediaExport::AudioOnly(AudioTarget::M4a(_)) => MediaExportKind::ToM4a,
            MediaExport::AudioOnly(AudioTarget::Mp3(_)) => MediaExportKind::ToMp3,
            MediaExport::AudioOnly(AudioTarget::Flac) => MediaExportKind::ToFlac,
            MediaExport::AudioOnly(AudioTarget::Wav) => MediaExportKind::ToWav,
            MediaExport::AudioOnly(AudioTarget::Opus(_)) => MediaExportKind::ToOpus,
        }
    }

    fn default_for(kind: MediaExportKind) -> Self {
        let bitrate = AudioTarget::default_bitrate();
        match kind {
            MediaExportKind::FramePng => MediaExport::CurrentFrame(RasterTarget::Png),
            MediaExportKind::FrameJpeg => MediaExport::CurrentFrame(RasterTarget::default_jpeg()),
            MediaExportKind::FrameWebp => MediaExport::CurrentFrame(RasterTarget::Webp),
            MediaExportKind::FrameAvif => MediaExport::CurrentFrame(RasterTarget::default_avif()),
            MediaExportKind::FrameTiff => MediaExport::CurrentFrame(RasterTarget::Tiff),
            MediaExportKind::Trim => MediaExport::Trim(TimeRange::WHOLE),
            MediaExportKind::ExtractAudio => MediaExport::AudioOnly(AudioTarget::Copy),
            MediaExportKind::ToM4a => MediaExport::AudioOnly(AudioTarget::M4a(bitrate)),
            MediaExportKind::ToMp3 => MediaExport::AudioOnly(AudioTarget::Mp3(bitrate)),
            MediaExportKind::ToFlac => MediaExport::AudioOnly(AudioTarget::Flac),
            MediaExportKind::ToWav => MediaExport::AudioOnly(AudioTarget::Wav),
            MediaExportKind::ToOpus => MediaExport::AudioOnly(AudioTarget::Opus(bitrate)),
        }
    }

    fn extension(&self) -> ExportExtension {
        match self {
            MediaExport::CurrentFrame(target) => target.extension(),
            MediaExport::Trim(_) => ExportExtension::Matching,
            MediaExport::AudioOnly(target) => target.extension(),
        }
    }
}
