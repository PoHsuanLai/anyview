//! Turning what the person asked for into an input for the stage that is showing: palette
//! commands and keys both end here, so a command means one thing wherever it came from.

use super::media::{MediaIn, MediaStage, StepDirection, TrackKind, TrimEdge};
use super::model::{Stage, StageIn, StageParams};
use super::pdf::{LineDir, PdfIn, PdfParams, PdfStage, end, nudged, start};
use super::raster::{RasterIn, RasterParams, RasterStage};
use super::table::{TableIn, TableStage};
use super::text::{TextIn, TextStage, TextStep};
use super::tree::{TreeIn, TreeStage};
use super::zoom::ZoomDir;
use crate::command::StageCommand;
use crate::typed::TypedText;
use anyview_core::{Speed, Zoom};

impl Stage {
    /// The input that carries out `command` on this stage, or `None` when this stage has no
    /// such command (a PDF has no source view, an image no playback to toggle).
    pub fn input_for(&self, command: StageCommand, params: &StageParams) -> Option<StageIn> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(_) => raster(command, &params.raster).map(StageIn::Raster),
            Stage::Pdf(stage) => pdf(command, stage, &params.pdf).map(StageIn::Pdf),
            Stage::Media(_) => media(command).map(StageIn::Media),
            Stage::Text(_) => text(command).map(StageIn::Text),
            Stage::Table(_) => table(command).map(StageIn::Table),
            Stage::Tree(_) => tree(command).map(StageIn::Tree),
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
            | Stage::Text(TextStage::Reading { .. })
            | Stage::Table(_)
            | Stage::Tree(_) => false,
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
            Stage::Table(TableStage::Selected { .. }) => Some(StageIn::Table(TableIn::Deselect)),
            Stage::Tree(TreeStage::Selected { .. }) => Some(StageIn::Tree(TreeIn::Deselect)),
            Stage::Table(TableStage::Browsing { .. }) | Stage::Tree(TreeStage::Browsing { .. }) => {
                None
            }
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
        | StageCommand::ScrollToEnd
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll => None,
    }
}

fn pdf(command: StageCommand, stage: &PdfStage, params: &PdfParams) -> Option<PdfIn> {
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
        StageCommand::LineUp => Some(PdfIn::GoTo(nudged(
            stage.place(),
            LineDir::Up,
            params.pages,
        ))),
        StageCommand::LineDown => Some(PdfIn::GoTo(nudged(
            stage.place(),
            LineDir::Down,
            params.pages,
        ))),
        StageCommand::ScrollToStart => Some(PdfIn::GoTo(start())),
        StageCommand::ScrollToEnd => Some(PdfIn::GoTo(end(params.pages))),
        StageCommand::DeletePage => Some(PdfIn::DeletePage),
        StageCommand::MovePageEarlier => Some(PdfIn::MovePage(StepDirection::Backward)),
        StageCommand::MovePageLater => Some(PdfIn::MovePage(StepDirection::Forward)),
        StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll => None,
    }
}

fn media(command: StageCommand) -> Option<MediaIn> {
    match command {
        StageCommand::TogglePlayback => Some(MediaIn::Toggle),
        StageCommand::SeekBack => Some(MediaIn::SeekBack),
        StageCommand::SeekForward => Some(MediaIn::SeekForward),
        StageCommand::SlowDown => Some(MediaIn::StepSpeed(StepDirection::Backward)),
        StageCommand::SpeedUp => Some(MediaIn::StepSpeed(StepDirection::Forward)),
        StageCommand::NormalSpeed => Some(MediaIn::SetSpeed(Speed::NORMAL)),
        StageCommand::NextChapter => Some(MediaIn::StepChapter(StepDirection::Forward)),
        StageCommand::PreviousChapter => Some(MediaIn::StepChapter(StepDirection::Backward)),
        StageCommand::NextAudioTrack => Some(MediaIn::CycleTrack(TrackKind::Audio)),
        StageCommand::NextSubtitles => Some(MediaIn::CycleTrack(TrackKind::Subtitles)),
        StageCommand::StepFrameForward => Some(MediaIn::FrameStep(StepDirection::Forward)),
        StageCommand::StepFrameBack => Some(MediaIn::FrameStep(StepDirection::Backward)),
        StageCommand::MarkTrimStart => Some(MediaIn::Mark(TrimEdge::Start)),
        StageCommand::MarkTrimEnd => Some(MediaIn::Mark(TrimEdge::End)),
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
        | StageCommand::ScrollToEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll => None,
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
        | StageCommand::SeekForward
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll => None,
    }
}

fn table(command: StageCommand) -> Option<TableIn> {
    match command {
        StageCommand::NextSheet => Some(TableIn::NextSheet),
        StageCommand::PreviousSheet => Some(TableIn::PreviousSheet),
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
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::NextPage
        | StageCommand::PreviousPage
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::CollapseAll => None,
    }
}

fn tree(command: StageCommand) -> Option<TreeIn> {
    match command {
        StageCommand::CollapseAll => Some(TreeIn::CollapseAll),
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
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::NextPage
        | StageCommand::PreviousPage
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet => None,
    }
}
