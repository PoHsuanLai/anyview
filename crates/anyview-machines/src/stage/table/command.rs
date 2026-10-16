//! What each stage command means for a table stage.

use super::{CellStep, TableIn, TableParams, TableStage};
use crate::command::StageCommand;
use crate::stage::data_edits::{EditSessionIn, TypingIn, TypingSeed};
use crate::stage::row::RowStep;
use crate::stage::text::{Changes, Editable};
use crate::stage::zoom::ZoomDir;
use crate::typed::TypedText;

/// The input that carries out `command` on a table stage, or `None` when it has no such command.
pub(in crate::stage) fn input_for(
    command: StageCommand,
    stage: &TableStage,
    params: &TableParams,
) -> Option<TableIn> {
    let editing = params.editable == Editable::Yes;
    let picked = stage.row().is_some();
    match command {
        StageCommand::NextSheet => Some(TableIn::NextSheet),
        StageCommand::PreviousSheet => Some(TableIn::PreviousSheet),
        StageCommand::LineUp => Some(TableIn::Move(RowStep::Up)),
        StageCommand::LineDown => Some(TableIn::Move(RowStep::Down)),
        StageCommand::PreviousPage => Some(TableIn::Move(RowStep::PageUp)),
        StageCommand::NextPage => Some(TableIn::Move(RowStep::PageDown)),
        StageCommand::ScrollToStart => Some(TableIn::Move(RowStep::Top)),
        StageCommand::ScrollToEnd => Some(TableIn::Move(RowStep::Bottom)),
        StageCommand::Find => Some(TableIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TableIn::NextHit),
        StageCommand::FindPrevious => Some(TableIn::PreviousHit),
        StageCommand::ZoomIn => Some(TableIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(TableIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToActual => Some(TableIn::ZoomActual),
        StageCommand::SelectAll => Some(TableIn::SelectAll),
        StageCommand::CopyRows if stage.selected_count() > 0 => Some(TableIn::Copy),
        StageCommand::Edit if editing && picked => {
            Some(TableIn::Typing(TypingIn::Begin(TypingSeed::Existing)))
        }
        StageCommand::PreviousCell if editing && picked => Some(TableIn::StepCell(CellStep::Left)),
        StageCommand::NextCell if editing && picked => Some(TableIn::StepCell(CellStep::Right)),
        StageCommand::AddRow if editing && picked => Some(TableIn::AddRow),
        StageCommand::DeletePicked if editing && picked => Some(TableIn::DeleteRows),
        StageCommand::Save if stage.cells().edits.log.changes == Changes::Unsaved => {
            Some(TableIn::Session(EditSessionIn::Save))
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
        | StageCommand::CollapseAll
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
