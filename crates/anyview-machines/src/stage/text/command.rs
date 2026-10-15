//! What each stage command means for a text stage.

use super::{Editable, TextIn, TextParams, TextStage, TextStep};
use crate::command::StageCommand;
use crate::stage::zoom::ZoomDir;
use crate::typed::TypedText;

/// The input that carries out `command` on a text stage, or `None` when it has no such command.
pub(in crate::stage) fn input_for(
    command: StageCommand,
    stage: &TextStage,
    params: &TextParams,
) -> Option<TextIn> {
    let editing = stage.edited().is_some();
    match command {
        StageCommand::Find => Some(TextIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TextIn::NextHit),
        StageCommand::FindPrevious => Some(TextIn::PreviousHit),
        StageCommand::ToggleSource if !editing => Some(TextIn::ToggleSource),
        StageCommand::ToggleWrap => Some(TextIn::ToggleWrap),
        StageCommand::ZoomIn => Some(TextIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(TextIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToActual => Some(TextIn::ZoomActual),
        StageCommand::Edit if !editing && params.editable == Editable::Yes => Some(TextIn::Edit),
        StageCommand::Done if editing => Some(TextIn::Done),
        StageCommand::Save if editing => Some(TextIn::Save),
        StageCommand::NextPage => Some(TextIn::Step(TextStep::PageDown)),
        StageCommand::PreviousPage => Some(TextIn::Step(TextStep::PageUp)),
        StageCommand::LineUp => Some(TextIn::Step(TextStep::LineUp)),
        StageCommand::LineDown => Some(TextIn::Step(TextStep::LineDown)),
        StageCommand::ScrollToStart => Some(TextIn::Step(TextStep::Top)),
        StageCommand::ScrollToEnd => Some(TextIn::Step(TextStep::Bottom)),
        StageCommand::ToggleSource
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
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
        | StageCommand::CollapseAll
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}
