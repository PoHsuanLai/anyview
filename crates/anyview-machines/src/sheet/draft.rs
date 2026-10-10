//! What the export sheet holds while it is open: one format's export choice.
//!
//! The sheet is an enum of sheets rather than a type generic over `ExportChoice`: which format
//! the sheet exports is known only when it opens (the next file in the sequence may be another
//! kind), so a type parameter would spread through the viewer's root, its inputs and its outputs
//! for nothing. The four `ExportChoice` types are a closed set, so they are an enum, and the one
//! generic function below does for each of them what the trait is for.

use super::offer::MediaOffer;
use super::option::requality;
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

/// A row chosen in the dialog's list of formats.
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
    /// options. `None` for a family with no kinds, and for media, whose kinds depend on what is
    /// installed (`MediaOffer::first`).
    pub fn first_of(family: ExportFamily) -> Option<ExportDraft> {
        match family {
            ExportFamily::Raster => first::<RasterExport>().map(ExportDraft::Raster),
            ExportFamily::Pdf => first::<PdfExport>().map(ExportDraft::Pdf),
            ExportFamily::Text => first::<TextExport>().map(ExportDraft::Text),
            ExportFamily::Media => None,
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

    /// The draft after the list's row for `pick` was chosen, with that kind's default options; a
    /// picture keeps its size, its metadata choice and the quality it had, since only the
    /// format changed. `None` when the pick belongs to another format.
    pub fn picked(self, pick: ExportKindPick) -> Option<ExportDraft> {
        match (self, pick) {
            (ExportDraft::Raster(before), ExportKindPick::Raster(kind)) => {
                Some(ExportDraft::Raster(carried(before, default_of(kind))))
            }
            (ExportDraft::Pdf(_), ExportKindPick::Pdf(kind)) => {
                Some(ExportDraft::Pdf(default_of(kind)))
            }
            (ExportDraft::Text(_), ExportKindPick::Text(kind)) => {
                Some(ExportDraft::Text(default_of(kind)))
            }
            (ExportDraft::Media(before), ExportKindPick::Media(kind)) => {
                Some(ExportDraft::Media(frame_quality(before, default_of(kind))))
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

impl ExportDraft {
    /// The kind this draft is, as the list's current row.
    pub fn pick(self) -> ExportKindPick {
        match self {
            ExportDraft::Raster(choice) => ExportKindPick::Raster(choice.kind()),
            ExportDraft::Pdf(choice) => ExportKindPick::Pdf(choice.kind()),
            ExportDraft::Text(choice) => ExportKindPick::Text(choice.kind()),
            ExportDraft::Media(choice) => ExportKindPick::Media(choice.kind()),
        }
    }

    /// Every kind the dialog lists for this draft's format; the words are
    /// [`kind_name`](super::words::kind_name) and [`kind_hint`](super::words::kind_hint).
    pub fn choices(self) -> Vec<ExportKindPick> {
        match self {
            ExportDraft::Raster(_) => listed::<RasterExport>(ExportKindPick::Raster),
            ExportDraft::Pdf(_) => listed::<PdfExport>(ExportKindPick::Pdf),
            ExportDraft::Text(_) => listed::<TextExport>(ExportKindPick::Text),
            ExportDraft::Media(_) => listed::<MediaExport>(ExportKindPick::Media),
        }
    }
}

impl ExportDraft {
    /// [`ExportDraft::choices`] without the media kinds `offer` does not hold; another format's
    /// list is whole.
    pub fn choices_within(self, offer: &MediaOffer) -> Vec<ExportKindPick> {
        self.choices()
            .into_iter()
            .filter(|pick| match pick {
                ExportKindPick::Media(kind) => offer.offers(*kind),
                ExportKindPick::Raster(_) | ExportKindPick::Pdf(_) | ExportKindPick::Text(_) => {
                    true
                }
            })
            .collect()
    }
}

fn listed<E: ExportChoice>(wrap: fn(E::Kind) -> ExportKindPick) -> Vec<ExportKindPick> {
    E::kinds().iter().map(|kind| wrap(*kind)).collect()
}

/// `next`, a picture's export in a newly chosen format, with the size and metadata choice of
/// `before` and its quality where both are lossy.
fn carried(before: RasterExport, next: RasterExport) -> RasterExport {
    match (before, next) {
        (RasterExport::Image(was, resize, keep), RasterExport::Image(target, ..)) => {
            RasterExport::Image(requality(target, was), resize, keep)
        }
        (RasterExport::Image(..) | RasterExport::Pdf, next) => next,
    }
}

/// `next`, a recording's export in a newly chosen kind, with the quality of the frame `before`
/// was when both are frames in a lossy format.
fn frame_quality(before: MediaExport, next: MediaExport) -> MediaExport {
    match (before, next) {
        (MediaExport::CurrentFrame(was), MediaExport::CurrentFrame(target)) => {
            MediaExport::CurrentFrame(requality(target, was))
        }
        (_, next) => next,
    }
}

fn first<E: ExportChoice>() -> Option<E> {
    E::kinds().first().map(|kind| E::default_for(*kind))
}

fn default_of<E: ExportChoice>(kind: E::Kind) -> E {
    E::default_for(kind)
}
