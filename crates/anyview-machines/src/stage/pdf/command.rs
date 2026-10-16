//! What each stage command means for a PDF stage.

use super::{LineDir, PageEdits, PdfIn, PdfParams, PdfStage, end, nudged, start};
use crate::command::StageCommand;
use crate::stage::media::StepDirection;
use crate::stage::zoom::ZoomDir;
use crate::typed::TypedText;
use anyview_core::Zoom;

/// The input that carries out `command` on a PDF stage, or `None` when it has no such command.
pub(in crate::stage) fn input_for(
    command: StageCommand,
    stage: &PdfStage,
    params: &PdfParams,
) -> Option<PdfIn> {
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
        StageCommand::DeletePage if params.page_edits == PageEdits::Allowed => {
            Some(PdfIn::DeletePage)
        }
        StageCommand::MovePageEarlier if params.page_edits == PageEdits::Allowed => {
            Some(PdfIn::MovePage(StepDirection::Backward))
        }
        StageCommand::MovePageLater if params.page_edits == PageEdits::Allowed => {
            Some(PdfIn::MovePage(StepDirection::Forward))
        }
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
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
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
