//! What each stage command means for a picture stage.

use super::{RasterIn, RasterParams, RasterStage};
use crate::command::StageCommand;
use crate::stage::media::StepDirection;
use crate::stage::zoom::ZoomDir;
use anyview_core::Zoom;

/// The input that carries out `command` on a picture stage, or `None` when it has no such command.
pub(in crate::stage) fn input_for(
    command: StageCommand,
    stage: &RasterStage,
    params: &RasterParams,
) -> Option<RasterIn> {
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
        StageCommand::TogglePlayback if stage.is_animated() => Some(RasterIn::TogglePlayback),
        StageCommand::StepFrameForward if stage.is_animated() => {
            Some(RasterIn::StepFrame(StepDirection::Forward))
        }
        StageCommand::StepFrameBack if stage.is_animated() => {
            Some(RasterIn::StepFrame(StepDirection::Backward))
        }
        StageCommand::TogglePlayback
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::ZoomToWidth
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
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}
