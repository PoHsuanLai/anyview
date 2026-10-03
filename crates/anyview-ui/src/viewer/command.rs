//! What a palette row does: the root decides which region a command belongs to.

use super::model::{Viewer, ViewerOut, ViewerParams};
use super::region::{Step, presentation, sheet, stage};
use crate::command::Command;
use crate::presentation::PresentationIn;
use crate::sheet::{ExportDraft, ExportFamily, SheetIn};
use crate::stage::{RasterIn, Spin, Stage, StageIn};
use anyview_core::FileAction;
use ds_core::time::stamp::Stamp;

/// The export format of what the stage shows, or `None` when the file has no export.
fn export_family(stage: &Stage) -> Option<ExportFamily> {
    match stage {
        Stage::NoStage => None,
        Stage::Raster(_) => Some(ExportFamily::Raster),
        Stage::Pdf(_) => Some(ExportFamily::Pdf),
        Stage::Media(_) => Some(ExportFamily::Media),
        Stage::Text(_) => Some(ExportFamily::Text),
    }
}

pub(super) fn run(viewer: Viewer, command: Command, at: Stamp, params: &ViewerParams) -> Step {
    match command {
        Command::Stage(command) => match viewer.stage.input_for(command, &params.stage) {
            Some(input) => stage(viewer, input, at, params),
            None => (viewer, vec![]),
        },
        Command::File(action) => file_action(viewer, action, at, params),
    }
}

/// The export sheet for what the stage shows. A recording's depends on what is installed: with
/// nothing on offer the sheet says which package adds the exports.
fn export(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let opening = match export_family(&viewer.stage) {
        None => None,
        Some(ExportFamily::Media) => {
            let offer = &params.sheet.media;
            match (offer.first(), offer.needs()) {
                (Some(choice), _) => Some(SheetIn::OpenExport(ExportDraft::Media(choice))),
                (None, Some(needs)) => Some(SheetIn::OpenUnavailable(needs.clone())),
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

/// Export, trash and the mini window are the viewer's own to start; a turn goes to an image
/// stage; every other action is the edge's.
fn file_action(viewer: Viewer, action: FileAction, at: Stamp, params: &ViewerParams) -> Step {
    let handed_over = |viewer: Viewer| (viewer, vec![ViewerOut::Run(action)]);
    match action {
        FileAction::Export => export(viewer, at, params),
        FileAction::MoveToTrash => sheet(viewer, SheetIn::AskTrash, at, params),
        FileAction::PlayInMiniWindow => presentation(viewer, PresentationIn::ToMini, at, params),
        FileAction::RotateLeft => turn(viewer, Spin::Left, action, at, params),
        FileAction::RotateRight => turn(viewer, Spin::Right, action, at, params),
        FileAction::Open
        | FileAction::OpenWith
        | FileAction::RevealInFolder
        | FileAction::CopyFile
        | FileAction::CopyPath
        | FileAction::Share
        | FileAction::Rename
        | FileAction::Duplicate
        | FileAction::Print
        | FileAction::SaveCopy
        | FileAction::RevertTo
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical
        | FileAction::PlayInBackground
        | FileAction::ConvertTo => handed_over(viewer),
    }
}

/// A turn turns the image on screen; for anything else (a PDF page) it is the edge's.
fn turn(viewer: Viewer, spin: Spin, action: FileAction, at: Stamp, params: &ViewerParams) -> Step {
    match viewer.stage {
        Stage::Raster(_) => stage(viewer, StageIn::Raster(RasterIn::Rotate(spin)), at, params),
        Stage::NoStage | Stage::Pdf(_) | Stage::Media(_) | Stage::Text(_) => {
            (viewer, vec![ViewerOut::Run(action)])
        }
    }
}
