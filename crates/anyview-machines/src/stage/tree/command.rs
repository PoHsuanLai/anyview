//! What each stage command means for a tree stage.

use super::{KeyUnder, NodeUnder, TreeIn, TreeParams, TreeStage};
use crate::command::StageCommand;
use crate::stage::data_edits::{EditSessionIn, TypingIn, TypingSeed};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::text::{Changes, Editable};
use crate::stage::zoom::ZoomDir;
use crate::typed::TypedText;

/// The input that carries out `command` on a tree stage, or `None` when it has no such command.
pub(in crate::stage) fn input_for(
    command: StageCommand,
    stage: &TreeStage,
    params: &TreeParams,
) -> Option<TreeIn> {
    let editing = params.editable == Editable::Yes;
    let picked = stage.row();
    match command {
        StageCommand::CollapseAll => Some(TreeIn::CollapseAll),
        StageCommand::LineUp => Some(TreeIn::Move(RowStep::Up)),
        StageCommand::LineDown => Some(TreeIn::Move(RowStep::Down)),
        StageCommand::PreviousPage => Some(TreeIn::Move(RowStep::PageUp)),
        StageCommand::NextPage => Some(TreeIn::Move(RowStep::PageDown)),
        StageCommand::ScrollToStart => Some(TreeIn::Move(RowStep::Top)),
        StageCommand::ScrollToEnd => Some(TreeIn::Move(RowStep::Bottom)),
        StageCommand::Find => Some(TreeIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TreeIn::NextHit),
        StageCommand::FindPrevious => Some(TreeIn::PreviousHit),
        StageCommand::ZoomIn => Some(TreeIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(TreeIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToActual => Some(TreeIn::ZoomActual),
        StageCommand::Edit if editing && picked.is_some() && params.under == NodeUnder::Scalar => {
            Some(TreeIn::Typing(TypingIn::Begin(TypingSeed::Existing)))
        }
        StageCommand::RenameKey
            if editing && picked.is_some() && params.key == KeyUnder::Present =>
        {
            Some(TreeIn::RenameKey)
        }
        StageCommand::AddItem if editing && picked.is_some() => Some(TreeIn::AddItem),
        StageCommand::DeletePicked if editing && picked.is_some_and(|row| row != RowNo(0)) => {
            Some(TreeIn::DeleteItem)
        }
        StageCommand::Save if stage.edits().log.changes == Changes::Unsaved => {
            Some(TreeIn::Session(EditSessionIn::Save))
        }
        StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::ToggleSource
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
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
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
