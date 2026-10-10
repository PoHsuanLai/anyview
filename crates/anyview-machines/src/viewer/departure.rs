//! Going away from a file with changes that are not saved: a picture's edits or a text's. The
//! question is the same (Save, Don't Save, Cancel) and so is the held departure; what saving and
//! letting go mean is the open file's stage.

use super::command::run_stage;
use super::model::{Viewer, ViewerOut, ViewerParams};
use super::picture::{picture, save};
use super::region::{Step, sheet, stage};
use super::step::{begin, dropped, navigate_now};
use crate::SaveEnd;
use crate::command::StageCommand;
use crate::navigate::NavigateIn;
use crate::picture::PictureEditIn;
use crate::sheet::{Departure, SheetIn};
use crate::stage::{Changes, Outside, Stage, StageIn, TextIn};
use ds_core::time::stamp::Stamp;

/// Go where `departure` says, or first ask what to do with changes that are not saved.
pub(super) fn leaving(
    viewer: Viewer,
    departure: Departure,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    if viewer.unsaved() {
        sheet(viewer, SheetIn::AskUnsaved(departure), at, params)
    } else {
        leave(viewer, departure, at, params)
    }
}

/// Go where `departure` says.
pub(super) fn leave(
    viewer: Viewer,
    departure: Departure,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    match departure {
        Departure::Close => (viewer, vec![ViewerOut::CloseWindow]),
        Departure::Open(path) => begin(viewer, &path, at, params),
        Departure::Chosen(paths) | Departure::Dropped(paths) => dropped(viewer, paths, at, params),
        Departure::Walk(input) => navigate_now(viewer, input, at, params),
        Departure::Finish => run_stage(viewer, StageCommand::Done, at, params),
    }
}

/// Whether `input` walks to another file, so that it would leave the file shown.
pub(super) fn walks(input: &NavigateIn) -> bool {
    match input {
        NavigateIn::Next | NavigateIn::Previous | NavigateIn::First | NavigateIn::Last => true,
        NavigateIn::Start(_) | NavigateIn::Leave | NavigateIn::Gone | NavigateIn::Elapsed => false,
    }
}

/// Save what the open file's stage has changed: a picture's edits, or the text being edited.
pub(super) fn save_open(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    match viewer.stage {
        Stage::Text(_) => save_text(viewer, at, params),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Table(_)
        | Stage::Tree(_) => save(viewer, at, params),
    }
}

/// ⌘S on a text being edited: the text is written, unless another program changed the file, in
/// which case the person is asked before it is replaced.
pub(super) fn save_text(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let outside = viewer
        .stage
        .edited()
        .is_some_and(|edited| edited.outside == Outside::Changed);
    if outside {
        return sheet(viewer, SheetIn::AskReplace, at, params);
    }
    run_stage(viewer, StageCommand::Save, at, params)
}

/// Don't Save: the changes are let go.
pub(super) fn discard(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    match viewer.stage {
        Stage::Text(_) => stage(
            viewer,
            StageIn::Text(TextIn::Edited(Changes::Saved)),
            at,
            params,
        ),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Table(_)
        | Stage::Tree(_) => picture(viewer, PictureEditIn::Discard, at, params),
    }
}

/// The host's save of the edited text ended: where the person was going when they chose to save
/// first is where they go now if the text is in the file, and nowhere if it is not.
pub(super) fn saved(viewer: Viewer, end: SaveEnd, at: Stamp, params: &ViewerParams) -> Step {
    let mut viewer = viewer;
    let going = viewer.after_save.take();
    match (end, going) {
        (SaveEnd::Written, Some(departure)) => leaving(viewer, departure, at, params),
        (SaveEnd::Written | SaveEnd::Refused, _) => (viewer, Vec::new()),
    }
}
