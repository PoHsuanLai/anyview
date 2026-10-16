//! A key press: routed to the region that wants it, and the undo and dismiss it can mean.

use super::command::run;
use super::departure::{leaving, save_open};
use super::find::{open_find, palette};
use super::markup::{markup_in, markup_key};
use super::model::{Viewer, ViewerOut, ViewerParams};
use super::opening::choose;
use super::picture::{crop_key, picture};
use super::region::{Step, chrome, panel, stage};
use super::step::{context, hand, navigate, sheet_in};
use crate::command::{Command, StageCommand};
use crate::context::ContextIn;
use crate::edits::Rewind;
use crate::keys::{Act, Chords, Press, Regions, Route, route};
use crate::markup::MarkupIn;
use crate::palette::PaletteIn;
use crate::picture::PictureEditIn;
use crate::presentation::Presentation;
use crate::sheet::Departure;
use crate::stage::{Stage, StageIn, TextIn};
use crate::trim::TrimIn;
use ds_core::time::stamp::Stamp;

pub(super) fn keyed(viewer: Viewer, key: &Press, at: Stamp, params: &ViewerParams) -> Step {
    if let Some(input) = crop_key(&viewer, key.keys().as_slice()) {
        return picture(viewer, input, at, params);
    }
    if let Some(input) = super::trim::trim_key(&viewer, key.keys().as_slice()) {
        return super::trim::trim_in(viewer, input, at, params);
    }
    if let Some(input) = markup_key(&viewer, key.keys().as_slice()) {
        return markup_in(viewer, input, at, params);
    }
    let routed = route(
        key,
        Regions {
            sheet: &viewer.sheet,
            palette: &viewer.palette,
            context: &viewer.context,
            panel: &viewer.panel,
            stage: &viewer.stage,
            stage_params: &params.stage,
            platform: params.platform,
            chords: match viewer.presentation {
                Presentation::Pane => Chords::None,
                Presentation::Window
                | Presentation::Peek
                | Presentation::Mini
                | Presentation::Background => Chords::Viewer,
            },
        },
    );
    match routed {
        Route::Sheet(input) => sheet_in(viewer, input, at, params),
        Route::Palette(input) => palette(viewer, input, at, params),
        Route::OpenPalette => palette(viewer, PaletteIn::Open, at, params),
        Route::OpenFind => open_find(viewer, at, params),
        Route::Context(input) => context(viewer, input, at, params),
        Route::OpenContextMenu => context(viewer, ContextIn::OpenAtCentre, at, params),
        Route::Panel(input) => panel(viewer, input, at, params),
        Route::CloseWindow => leaving(viewer, Departure::Close, at, params),
        Route::Save => save_open(viewer, at, params),
        Route::OpenFile => choose(viewer),
        Route::Rewind(rewind) => rewound(viewer, rewind, at, params),
        Route::Dismiss => dismissed(viewer, at, params),
        // Done and Save of a text being edited go through the command, which asks first where a
        // question is due.
        Route::Stage(StageIn::Text(TextIn::Done)) => {
            run(viewer, Command::Stage(StageCommand::Done), at, params)
        }
        Route::Stage(StageIn::Text(TextIn::Save)) => {
            run(viewer, Command::Stage(StageCommand::Save), at, params)
        }
        Route::Stage(input) => stage(viewer, input, at, params),
        Route::Hand(input) => (hand(viewer, input), vec![]),
        Route::Navigate(input) => navigate(viewer, input, at, params),
        Route::Chrome(input) => chrome(viewer, input, at, params),
        Route::Ignored => file_key(viewer, key, at, params),
        Route::Swallowed => (viewer, vec![]),
    }
}

/// A press no region claimed, which may be the action of a file action the open file offers (Print,
/// Duplicate, Rotate…): it runs as the palette's row would.
fn file_key(viewer: Viewer, key: &Press, at: Stamp, params: &ViewerParams) -> Step {
    match key.act() {
        Some(Act::File(action)) if params.files.contains(&action) => {
            run(viewer, Command::File(action), at, params)
        }
        Some(_) | None => (viewer, vec![]),
    }
}

/// ⌘Z and ⇧⌘Z: a trim, an edit of the picture, a field or mark of the PDF, or an edit of a table's
/// cells or a tree's nodes that is not saved is
/// taken back or done again first; with none, it is the file's last save that is.
fn rewound(viewer: Viewer, rewind: Rewind, at: Stamp, params: &ViewerParams) -> Step {
    let trimmed = match rewind {
        Rewind::Undo if viewer.trim.can_undo() => Some(TrimIn::Undo),
        Rewind::Redo if viewer.trim.can_redo() => Some(TrimIn::Redo),
        Rewind::Undo | Rewind::Redo => None,
    };
    if let Some(input) = trimmed {
        return super::trim::trim_in(viewer, input, at, params);
    }
    let on_pages = matches!(viewer.stage, Stage::Pdf(_));
    match rewind {
        Rewind::Undo if on_pages && viewer.markup.can_undo() => {
            return markup_in(viewer, MarkupIn::Undo, at, params);
        }
        Rewind::Redo if on_pages && viewer.markup.can_redo() => {
            return markup_in(viewer, MarkupIn::Redo, at, params);
        }
        Rewind::Undo | Rewind::Redo => {}
    }
    let input = match rewind {
        Rewind::Undo if viewer.picture.can_undo() => Some(PictureEditIn::Undo),
        Rewind::Redo if viewer.picture.can_redo() => Some(PictureEditIn::Redo),
        Rewind::Undo | Rewind::Redo => None,
    };
    match (input, viewer.stage.rewind_input(rewind)) {
        (Some(input), _) => picture(viewer, input, at, params),
        // Then the edits of a table's cells or a tree's nodes that are not saved.
        (None, Some(input)) => stage(viewer, input, at, params),
        (None, None) => (viewer, vec![ViewerOut::Rewind(rewind)]),
    }
}

/// Esc with nothing open: a quick look closes; a window, a mini window and a background session
/// stay; a pane gives the keyboard back to its host and stays.
fn dismissed(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    match viewer.presentation {
        Presentation::Peek => leaving(viewer, Departure::Close, at, params),
        Presentation::Pane => (viewer, vec![ViewerOut::Unfocus]),
        Presentation::Window | Presentation::Mini | Presentation::Background => (viewer, vec![]),
    }
}
