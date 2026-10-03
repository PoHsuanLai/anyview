//! The stage region as the viewer's root sees it: one of the four stage machines, or none.

use super::family::StageFamily;
use super::media::{MediaIn, MediaOut, MediaParams, MediaStage};
use super::pdf::{PdfIn, PdfOut, PdfParams, PdfStage};
use super::raster::{RasterIn, RasterOut, RasterParams, RasterStage};
use super::text::{TextIn, TextOut, TextParams, TextStage, TextViews};

/// What shows the open file's content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Stage {
    /// Nothing yet, or a file with no stage (`StageFamily::PeekOnly`): facts and Open With….
    #[default]
    NoStage,
    /// An image.
    Raster(RasterStage),
    /// A PDF.
    Pdf(PdfStage),
    /// Video or audio.
    Media(MediaStage),
    /// Text, code or Markdown.
    Text(TextStage),
}

impl Stage {
    /// A new stage for a file of `family`; `views` says which views a text file has.
    pub fn for_family(family: StageFamily, views: TextViews) -> Stage {
        match family {
            StageFamily::Raster => Stage::Raster(RasterStage::default()),
            StageFamily::Pdf => Stage::Pdf(PdfStage::default()),
            StageFamily::Media => Stage::Media(MediaStage::default()),
            StageFamily::Text => Stage::Text(TextStage::opened(views)),
            StageFamily::PeekOnly => Stage::NoStage,
        }
    }
}

/// An input for whichever stage is showing. An input for another family's stage means a result
/// that arrived after the file changed, and is ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageIn {
    /// For the raster stage.
    Raster(RasterIn),
    /// For the PDF stage.
    Pdf(PdfIn),
    /// For the media stage.
    Media(MediaIn),
    /// For the text stage.
    Text(TextIn),
    /// The clock, for every stage.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for StageIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        StageIn::Elapsed
    }
}

/// What the showing stage wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageOut {
    /// From the raster stage.
    Raster(RasterOut),
    /// From the PDF stage.
    Pdf(PdfOut),
    /// From the media stage.
    Media(MediaOut),
    /// From the text stage.
    Text(TextOut),
}

/// What each stage needs from the view and settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageParams {
    /// For the raster stage.
    pub raster: RasterParams,
    /// For the PDF stage.
    pub pdf: PdfParams,
    /// For the media stage.
    pub media: MediaParams,
    /// For the text stage.
    pub text: TextParams,
}
