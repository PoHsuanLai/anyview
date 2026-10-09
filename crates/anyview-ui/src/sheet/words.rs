//! The words the export sheet says about a format: its name, one line of what it is for, and
//! the name the file will be saved under.

use super::draft::{ExportDraft, ExportKindPick};
use anyview_core::{
    ExportChoice, ExportExtension, MediaExportKind, PdfExportKind, RasterExportKind, TextExportKind,
};
use ds_core::word::Word;

/// What the format's row is called.
pub fn kind_name(pick: ExportKindPick) -> &'static str {
    match pick {
        ExportKindPick::Raster(kind) => match kind {
            RasterExportKind::Png => "PNG",
            RasterExportKind::Jpeg => "JPEG",
            RasterExportKind::Webp => "WebP",
            RasterExportKind::Avif => "AVIF",
            RasterExportKind::Tiff => "TIFF",
            RasterExportKind::Pdf => "PDF",
        },
        ExportKindPick::Pdf(kind) => match kind {
            PdfExportKind::Pdf => "PDF",
            PdfExportKind::PageImages => "Images",
            PdfExportKind::PlainText => "Plain Text",
            PdfExportKind::Markdown => "Markdown",
        },
        ExportKindPick::Text(kind) => match kind {
            TextExportKind::Pdf => "PDF",
            TextExportKind::PlainText => "Plain Text",
        },
        ExportKindPick::Media(kind) => match kind {
            MediaExportKind::FramePng => "Frame as PNG",
            MediaExportKind::FrameJpeg => "Frame as JPEG",
            MediaExportKind::FrameWebp => "Frame as WebP",
            MediaExportKind::FrameAvif => "Frame as AVIF",
            MediaExportKind::FrameTiff => "Frame as TIFF",
            MediaExportKind::Trim => "Trim",
            MediaExportKind::ExtractAudio => "Audio as it is",
            MediaExportKind::ToM4a => "M4A",
            MediaExportKind::ToMp3 => "MP3",
            MediaExportKind::ToFlac => "FLAC",
            MediaExportKind::ToWav => "WAV",
            MediaExportKind::ToOpus => "Opus",
        },
    }
}

/// One line of what the format is for.
pub fn kind_hint(pick: ExportKindPick) -> &'static str {
    match pick {
        ExportKindPick::Raster(kind) => match kind {
            RasterExportKind::Png => "Sharp, and keeps transparency.",
            RasterExportKind::Jpeg => "Smaller files, with no transparency.",
            RasterExportKind::Webp => "Sharp and compact.",
            RasterExportKind::Avif => "The smallest files, in a newer format.",
            RasterExportKind::Tiff => "Sharp, for editing and print.",
            RasterExportKind::Pdf => "One page holding the picture.",
        },
        ExportKindPick::Pdf(kind) => match kind {
            PdfExportKind::Pdf => "A new PDF of some or all of the pages.",
            PdfExportKind::PageImages => "One picture for each page.",
            PdfExportKind::PlainText => "The words, with no layout.",
            PdfExportKind::Markdown => "The words, with headings and lists.",
        },
        ExportKindPick::Text(kind) => match kind {
            TextExportKind::Pdf => "Pages you can print, with text you can select.",
            TextExportKind::PlainText => "The text as it is.",
        },
        ExportKindPick::Media(kind) => match kind {
            MediaExportKind::FramePng
            | MediaExportKind::FrameJpeg
            | MediaExportKind::FrameWebp
            | MediaExportKind::FrameAvif
            | MediaExportKind::FrameTiff => "The picture on screen, at full size.",
            MediaExportKind::Trim => "Keep a part, without re-encoding it.",
            MediaExportKind::ExtractAudio => "The sound, untouched.",
            MediaExportKind::ToM4a => "AAC, which plays nearly everywhere.",
            MediaExportKind::ToMp3 => "Plays nearly everywhere.",
            MediaExportKind::ToFlac => "Compact, and loses nothing.",
            MediaExportKind::ToWav => "Uncompressed.",
            MediaExportKind::ToOpus => "Small and clear.",
        },
    }
}

impl ExportDraft {
    /// The extension of the file this draft writes.
    pub fn extension(self) -> ExportExtension {
        match self {
            ExportDraft::Raster(choice) => choice.extension(),
            ExportDraft::Pdf(choice) => choice.extension(),
            ExportDraft::Text(choice) => choice.extension(),
            ExportDraft::Media(choice) => choice.extension(),
        }
    }

    /// The name the export is saved under beside `original`: its name with the new extension. A
    /// trim or a copied track keeps the original's extension, as the back end does. `None` when
    /// the file has no name to build on, so there is nothing true to say.
    pub fn saved_as(self, original: &str) -> Option<String> {
        if original.is_empty() {
            return None;
        }
        let (stem, extension) = match original.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => (stem, Some(extension)),
            Some(_) | None => (original, None),
        };
        Some(match (self.extension(), extension) {
            (ExportExtension::Matching, Some(kept)) => format!("{stem}.{kept}"),
            (ExportExtension::Matching, None) => stem.to_owned(),
            (written, _) => format!("{stem}.{}", written.slug()),
        })
    }
}
