//! What the export sheet holds while it is open: one format's export choice.
//!
//! The sheet is an enum of sheets rather than a type generic over `ExportChoice`: which format
//! the sheet exports is known only when it opens (the next file in the sequence may be another
//! kind), so a type parameter would spread through the viewer's root, its inputs and its outputs
//! for nothing. The four `ExportChoice` types are a closed set, so they are an enum, and the one
//! generic function below does for each of them what the trait is for.

use anyview_core::{
    ExportChoice, MediaExport, MediaExportKind, PdfExport, PdfExportKind, RasterExport,
    RasterExportKind, TextExport, TextExportKind,
};
use ds_core::word::Word;

/// An export being chosen, of whichever format the file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportDraft {
    /// A raster or vector image.
    Raster(RasterExport),
    /// A PDF.
    Pdf(PdfExport),
    /// Markdown, code or plain text.
    Text(TextExport),
    /// A video or audio file.
    Media(MediaExport),
}

/// Which format an export is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ExportFamily {
    /// Images.
    Raster,
    /// PDFs.
    Pdf,
    /// Text documents.
    Text,
    /// Video and audio.
    Media,
}

/// An entry chosen in the sheet's format pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportKindPick {
    /// An image export kind.
    Raster(RasterExportKind),
    /// A PDF export kind.
    Pdf(PdfExportKind),
    /// A text export kind.
    Text(TextExportKind),
    /// A media export kind.
    Media(MediaExportKind),
}

impl ExportDraft {
    /// The draft the sheet opens on for `family`: its first kind with that kind's default
    /// options. `None` for a family with no kinds.
    pub fn first_of(family: ExportFamily) -> Option<ExportDraft> {
        match family {
            ExportFamily::Raster => first::<RasterExport>().map(ExportDraft::Raster),
            ExportFamily::Pdf => first::<PdfExport>().map(ExportDraft::Pdf),
            ExportFamily::Text => first::<TextExport>().map(ExportDraft::Text),
            ExportFamily::Media => first::<MediaExport>().map(ExportDraft::Media),
        }
    }

    /// The format this draft exports.
    pub fn family(self) -> ExportFamily {
        match self {
            ExportDraft::Raster(_) => ExportFamily::Raster,
            ExportDraft::Pdf(_) => ExportFamily::Pdf,
            ExportDraft::Text(_) => ExportFamily::Text,
            ExportDraft::Media(_) => ExportFamily::Media,
        }
    }

    /// The draft after the pop-up chose `pick`, with that kind's default options; `None` when
    /// the pick belongs to another format.
    pub fn picked(self, pick: ExportKindPick) -> Option<ExportDraft> {
        match (self, pick) {
            (ExportDraft::Raster(_), ExportKindPick::Raster(kind)) => {
                Some(ExportDraft::Raster(default_of(kind)))
            }
            (ExportDraft::Pdf(_), ExportKindPick::Pdf(kind)) => {
                Some(ExportDraft::Pdf(default_of(kind)))
            }
            (ExportDraft::Text(_), ExportKindPick::Text(kind)) => {
                Some(ExportDraft::Text(default_of(kind)))
            }
            (ExportDraft::Media(_), ExportKindPick::Media(kind)) => {
                Some(ExportDraft::Media(default_of(kind)))
            }
            (
                ExportDraft::Raster(_),
                ExportKindPick::Pdf(_) | ExportKindPick::Text(_) | ExportKindPick::Media(_),
            )
            | (
                ExportDraft::Pdf(_),
                ExportKindPick::Raster(_) | ExportKindPick::Text(_) | ExportKindPick::Media(_),
            )
            | (
                ExportDraft::Text(_),
                ExportKindPick::Raster(_) | ExportKindPick::Pdf(_) | ExportKindPick::Media(_),
            )
            | (
                ExportDraft::Media(_),
                ExportKindPick::Raster(_) | ExportKindPick::Pdf(_) | ExportKindPick::Text(_),
            ) => None,
        }
    }
}

fn first<E: ExportChoice>() -> Option<E> {
    E::kinds().first().map(|kind| E::default_for(*kind))
}

fn default_of<E: ExportChoice>(kind: E::Kind) -> E {
    E::default_for(kind)
}
