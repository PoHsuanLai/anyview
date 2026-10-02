//! Turning what the person asked for into an input for the stage that is showing: palette
//! commands and keys both end here, so a command means one thing wherever it came from.

use super::media::{MediaIn, MediaStage};
use super::model::{Stage, StageIn, StageParams};
use super::pdf::{PdfIn, PdfStage};
use super::raster::{RasterIn, RasterParams, RasterStage};
use super::text::{TextIn, TextStage, TextStep};
use super::zoom::ZoomDir;
use crate::command::StageCommand;
use crate::typed::TypedText;
use anyview_core::Zoom;

impl Stage {
    /// The input that carries out `command` on this stage, or `None` when this stage has no
    /// such command (a PDF has no source view, an image no playback to toggle).
    pub fn input_for(&self, command: StageCommand, params: &StageParams) -> Option<StageIn> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(_) => raster(command, &params.raster).map(StageIn::Raster),
            Stage::Pdf(_) => pdf(command).map(StageIn::Pdf),
            Stage::Media(_) => media(command).map(StageIn::Media),
            Stage::Text(_) => text(command).map(StageIn::Text),
        }
    }

    /// Whether a find is up: the keys go to its field, not to the window's chords.
    pub fn is_finding(&self) -> bool {
        match self {
            Stage::Pdf(PdfStage::Finding { .. }) | Stage::Text(TextStage::Finding { .. }) => true,
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Media(_)
            | Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. })
            | Stage::Text(TextStage::Reading { .. }) => false,
        }
    }

    /// The input that closes what the stage has open on top of its content (a find bar, a
    /// scrub), or `None` when it has nothing open: Esc undoes this before anything else.
    pub fn dismissal(&self) -> Option<StageIn> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(
                RasterStage::Fitted { .. }
                | RasterStage::Zoomed { .. }
                | RasterStage::Panning { .. },
            ) => None,
            Stage::Pdf(PdfStage::Finding { .. }) => Some(StageIn::Pdf(PdfIn::CloseFind)),
            Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. }) => None,
            Stage::Media(MediaStage::Scrubbing { .. }) => {
                Some(StageIn::Media(MediaIn::ScrubCancel))
            }
            Stage::Media(
                MediaStage::Opening
                | MediaStage::Playing { .. }
                | MediaStage::Paused { .. }
                | MediaStage::Ended { .. }
                | MediaStage::Failed(_),
            ) => None,
            Stage::Text(TextStage::Finding { .. }) => Some(StageIn::Text(TextIn::CloseFind)),
            Stage::Text(TextStage::Reading { .. }) => None,
        }
    }
}

fn raster(command: StageCommand, params: &RasterParams) -> Option<RasterIn> {
    let at = params.centre;
    match command {
        StageCommand::ZoomIn => Some(RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at,
        }),
        StageCommand::ZoomOut => Some(RasterIn::ZoomStep {
            dir: ZoomDir::Out,
            at,
        }),
        StageCommand::ZoomToFit => Some(RasterIn::SetZoom {
            zoom: Zoom::Fit,
            at,
        }),
        StageCommand::ZoomToActual => Some(RasterIn::SetZoom {
            zoom: Zoom::Actual,
            at,
        }),
        StageCommand::TogglePlayback => Some(RasterIn::TogglePlayback),
        StageCommand::ZoomToWidth
        | StageCommand::Find
        | StageCommand::FindNext
        | StageCommand::FindPrevious
        | StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::NextPage
        | StageCommand::PreviousPage
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd => None,
    }
}

fn pdf(command: StageCommand) -> Option<PdfIn> {
    match command {
        StageCommand::ZoomIn => Some(PdfIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(PdfIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToFit => Some(PdfIn::SetZoom(Zoom::Fit)),
        StageCommand::ZoomToWidth => Some(PdfIn::SetZoom(Zoom::Fill)),
        StageCommand::ZoomToActual => Some(PdfIn::SetZoom(Zoom::Actual)),
        StageCommand::Find => Some(PdfIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(PdfIn::NextHit),
        StageCommand::FindPrevious => Some(PdfIn::PreviousHit),
        StageCommand::NextPage => Some(PdfIn::NextPage),
        StageCommand::PreviousPage => Some(PdfIn::PreviousPage),
        StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd => None,
    }
}

fn media(command: StageCommand) -> Option<MediaIn> {
    match command {
        StageCommand::TogglePlayback => Some(MediaIn::Toggle),
        StageCommand::SeekBack => Some(MediaIn::SeekBack),
        StageCommand::SeekForward => Some(MediaIn::SeekForward),
        StageCommand::ZoomIn
        | StageCommand::ZoomOut
        | StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::ZoomToActual
        | StageCommand::Find
        | StageCommand::FindNext
        | StageCommand::FindPrevious
        | StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::NextPage
        | StageCommand::PreviousPage
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd => None,
    }
}

fn text(command: StageCommand) -> Option<TextIn> {
    match command {
        StageCommand::Find => Some(TextIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TextIn::NextHit),
        StageCommand::FindPrevious => Some(TextIn::PreviousHit),
        StageCommand::ToggleSource => Some(TextIn::ToggleSource),
        StageCommand::ToggleWrap => Some(TextIn::ToggleWrap),
        StageCommand::NextPage => Some(TextIn::Step(TextStep::PageDown)),
        StageCommand::PreviousPage => Some(TextIn::Step(TextStep::PageUp)),
        StageCommand::LineUp => Some(TextIn::Step(TextStep::LineUp)),
        StageCommand::LineDown => Some(TextIn::Step(TextStep::LineDown)),
        StageCommand::ScrollToStart => Some(TextIn::Step(TextStep::Top)),
        StageCommand::ScrollToEnd => Some(TextIn::Step(TextStep::Bottom)),
        StageCommand::ZoomIn
        | StageCommand::ZoomOut
        | StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::ZoomToActual
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward => None,
    }
}
