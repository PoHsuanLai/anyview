//! What each stage command means for a recording stage.

use super::{ControlOffer, MediaAbilities, MediaIn, StepDirection, TrackKind, TrimEdge};
use crate::command::StageCommand;
use anyview_core::Speed;

/// The input a media command is, or `None` when the player cannot do it.
pub(in crate::stage) fn input_for(
    command: StageCommand,
    abilities: &MediaAbilities,
) -> Option<MediaIn> {
    let needs = |control: ControlOffer, input: MediaIn| match control {
        ControlOffer::Offered => Some(input),
        ControlOffer::Withheld => None,
    };
    match command {
        StageCommand::TogglePlayback => Some(MediaIn::Toggle),
        StageCommand::SeekBack => Some(MediaIn::SeekBack),
        StageCommand::SeekForward => Some(MediaIn::SeekForward),
        StageCommand::SlowDown => {
            needs(abilities.speed, MediaIn::StepSpeed(StepDirection::Backward))
        }
        StageCommand::SpeedUp => needs(abilities.speed, MediaIn::StepSpeed(StepDirection::Forward)),
        StageCommand::NormalSpeed => needs(abilities.speed, MediaIn::SetSpeed(Speed::NORMAL)),
        StageCommand::NextChapter => needs(
            abilities.chapters,
            MediaIn::StepChapter(StepDirection::Forward),
        ),
        StageCommand::PreviousChapter => needs(
            abilities.chapters,
            MediaIn::StepChapter(StepDirection::Backward),
        ),
        StageCommand::NextAudioTrack => {
            needs(abilities.tracks, MediaIn::CycleTrack(TrackKind::Audio))
        }
        StageCommand::NextSubtitles => {
            needs(abilities.tracks, MediaIn::CycleTrack(TrackKind::Subtitles))
        }
        StageCommand::StepFrameForward => needs(
            abilities.frame_step,
            MediaIn::FrameStep(StepDirection::Forward),
        ),
        StageCommand::StepFrameBack => needs(
            abilities.frame_step,
            MediaIn::FrameStep(StepDirection::Backward),
        ),
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
