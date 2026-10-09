//! What a palette row does: the root decides which region a command belongs to.

use super::model::{Viewer, ViewerOut, ViewerParams};
use super::region::{Step, presentation, sheet, stage};
use crate::command::Command;
use crate::edits::{EditOffer, EditRequest};
use crate::presentation::PresentationIn;
use crate::sheet::{ExportDraft, ExportFamily, SheetIn};
use crate::stage::Stage;
use anyview_core::{Axis, Edit, FileAction, QuarterTurn};
use ds_core::time::stamp::Stamp;

pub(super) fn run(viewer: Viewer, command: Command, at: Stamp, params: &ViewerParams) -> Step {
    match command {
        Command::Stage(command) => match viewer.stage.input_for(command, &params.stage) {
            Some(input) => stage(viewer, input, at, params),
            None => (viewer, vec![]),
        },
        Command::File(action) => file_action(viewer, action, at, params),
        Command::OpenFile => super::step::choose(viewer),
        Command::Install(helper) => sheet(viewer, SheetIn::OfferHelper(helper), at, params),
    }
}

/// The export sheet for what the stage shows. A recording's depends on what is installed: with
/// nothing on offer the sheet says which package adds the exports.
fn export(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let opening = match viewer.stage.abilities().export {
        None => None,
        Some(ExportFamily::Media) => {
            let offer = &params.sheet.media;
            match (offer.first(), offer.needs()) {
                (Some(choice), _) => Some(SheetIn::OpenExport(ExportDraft::Media(choice))),
                (None, Some(needs)) => {
                    Some(SheetIn::OpenUnavailable(needs.clone(), offer.helper()))
                }
                (None, None) => None,
            }
        }
        Some(family) => ExportDraft::first_of(family).map(SheetIn::OpenExport),
    };
    match opening {
        Some(input) => sheet(viewer, input, at, params),
        None => (viewer, vec![]),
    }
}

/// Export and Convert To, trash, the mini window, the sheets of Rename, Save a Copy and Revert To and the edits are the
/// viewer's own to start; every other action is the edge's.
fn file_action(viewer: Viewer, action: FileAction, at: Stamp, params: &ViewerParams) -> Step {
    let handed_over = |viewer: Viewer| (viewer, vec![ViewerOut::Run(action)]);
    match action {
        // Converting is exporting: the sheet's pop-up is where the format is chosen.
        FileAction::Export | FileAction::ConvertTo => export(viewer, at, params),
        FileAction::Rename => (viewer, vec![ViewerOut::NameRename]),
        FileAction::MoveToTrash => sheet(viewer, SheetIn::AskTrash, at, params),
        FileAction::PlayInMiniWindow => presentation(viewer, PresentationIn::ToMini, at, params),
        FileAction::SaveCopy => (viewer, vec![ViewerOut::NameCopy]),
        FileAction::RevertTo => (viewer, vec![ViewerOut::ListVersions]),
        FileAction::RotateLeft => edit(
            viewer,
            Edit::Rotate(QuarterTurn::ThreeQuarter),
            action,
            at,
            params,
        ),
        FileAction::RotateRight => edit(
            viewer,
            Edit::Rotate(QuarterTurn::Quarter),
            action,
            at,
            params,
        ),
        FileAction::FlipHorizontal => {
            edit(viewer, Edit::Flip(Axis::Horizontal), action, at, params)
        }
        FileAction::FlipVertical => edit(viewer, Edit::Flip(Axis::Vertical), action, at, params),
        FileAction::Open
        | FileAction::RevealInFolder
        | FileAction::CopyFile
        | FileAction::CopyPath
        | FileAction::Share
        | FileAction::Duplicate
        | FileAction::Print
        | FileAction::PlayInBackground => handed_over(viewer),
    }
}

/// An edit of what the stage shows: a picture's, or a PDF's page the person is on. Anything else
/// has no such edit, and the host says so. An edit that loses something is asked about first, and
/// one the file cannot take at all does nothing.
fn edit(viewer: Viewer, edit: Edit, action: FileAction, at: Stamp, params: &ViewerParams) -> Step {
    let asked = match &viewer.stage {
        Stage::Raster(_) => Some(EditRequest::of_picture(edit)),
        Stage::Pdf(stage) => Some(EditRequest::on_page(edit, stage.place().page)),
        Stage::NoStage | Stage::Media(_) | Stage::Text(_) | Stage::Table(_) | Stage::Tree(_) => {
            None
        }
    };
    match (asked, params.sheet.edit) {
        (Some(_), EditOffer::Withheld) => (viewer, vec![]),
        (Some(request), EditOffer::Asks(caution)) => {
            sheet(viewer, SheetIn::AskEdit(request, caution), at, params)
        }
        (Some(request), EditOffer::Plain) => (viewer, vec![ViewerOut::Edit(request)]),
        (None, _) => (viewer, vec![ViewerOut::Run(action)]),
    }
}
