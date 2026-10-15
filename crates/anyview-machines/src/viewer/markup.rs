//! The viewer's side of writing on a PDF: the markup stepped, what it asks of the stage (the page
//! a field is on is shown), the pointer tool it follows, saving what was written, and the keys it
//! takes while something is picked or a field has the keyboard.

use super::model::{Viewer, ViewerOut, ViewerParams};
use super::region::{Step, sheet, stage};
use super::step::hand;
use crate::command::MarkupCommand;
use crate::context::ContextMenu;
use crate::edits::{EditOffer, EditRequest};
use crate::hand::{HandIn, Tool};
use crate::markup::{MarkupIn, MarkupOut, Using};
use crate::palette::Palette;
use crate::sheet::{Sheet, SheetIn, SheetOut};
use crate::stage::{Destination, PdfIn, PdfOut, Stage, StageIn, StageOut};
use anyview_core::Edit;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::ShortcutKey;

/// An input of the markup. Where it asks for a page to be shown, the stage is told to go there.
pub(super) fn markup_in(viewer: Viewer, input: MarkupIn, at: Stamp, params: &ViewerParams) -> Step {
    let (markup, outs) = viewer.markup.step(input);
    let mut state = (Viewer { markup, ..viewer }, Vec::new());
    for out in outs {
        let (viewer, mut outs) = state;
        let (viewer, more) = match out {
            MarkupOut::Reveal { page, offset } => stage(
                viewer,
                StageIn::Pdf(PdfIn::GoTo(Destination { page, offset })),
                at,
                params,
            ),
        };
        outs.extend(more);
        state = (viewer, outs);
    }
    state
}

/// A command for the marks. A tool chosen while words are selected marks them and leaves the
/// pointer as it was; chosen with none selected, it turns the Markup tool on with that tool.
pub(super) fn markup_command(
    viewer: Viewer,
    command: MarkupCommand,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    match command {
        MarkupCommand::Tool(tool) if tool.marks_words() && viewer.markup.has_selection() => {
            markup_in(viewer, MarkupIn::Apply(tool), at, params)
        }
        MarkupCommand::Tool(tool) => {
            let viewer = hand(viewer, HandIn::Use(Tool::Markup));
            markup_in(viewer, MarkupIn::Tool(tool), at, params)
        }
        MarkupCommand::Color(color) => markup_in(viewer, MarkupIn::Color(color), at, params),
        MarkupCommand::Delete => markup_in(viewer, MarkupIn::Delete, at, params),
        MarkupCommand::Save => save(viewer, at, params),
    }
}

/// ⌘S on a PDF: the fields filled and the marks made are written into the file. A signed document
/// asks first; one with nothing written does nothing; one that cannot be written to offers nothing.
pub(super) fn save(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let (viewer, mut outs) = markup_in(viewer, MarkupIn::Done, at, params);
    let page = match &viewer.stage {
        Stage::Pdf(pdf) => Some(pdf.place().page),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    };
    let Some(page) = page.filter(|_| viewer.markup.is_edited()) else {
        return (viewer, outs);
    };
    let request = EditRequest::on_page(Edit::Markup, page);
    match params.sheet.edit {
        EditOffer::Plain => {
            outs.push(ViewerOut::Edit(request));
            (viewer, outs)
        }
        EditOffer::Asks(caution) => {
            let (viewer, more) = sheet(viewer, SheetIn::AskEdit(request, caution), at, params);
            outs.extend(more);
            (viewer, outs)
        }
        EditOffer::Withheld | EditOffer::Copies(_) => (viewer, outs),
    }
}

/// What the markup takes the pointer for follows the pointer tool: on a PDF, Select selects words
/// and Markup draws; anything else reads.
pub(super) fn markup_synced(viewer: Viewer) -> Viewer {
    let wants = match (&viewer.stage, viewer.hand.tool) {
        (Stage::Pdf(_), Tool::Select) => Using::Selecting,
        (Stage::Pdf(_), Tool::Markup) => Using::Marking,
        (Stage::Pdf(_), Tool::Pan | Tool::Crop)
        | (
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Table(_)
            | Stage::Tree(_),
            Tool::Select | Tool::Pan | Tool::Crop | Tool::Markup,
        ) => Using::Reading,
    };
    if viewer.markup.using() == wants {
        return viewer;
    }
    let (markup, _) = viewer.markup.step(MarkupIn::Using(wants));
    Viewer { markup, ..viewer }
}

/// What Esc, Delete and Tab mean while something is picked, selected or written, or a field has
/// the keyboard, and nothing else has the keys.
pub(super) fn markup_key(viewer: &Viewer, keys: &[ShortcutKey]) -> Option<MarkupIn> {
    let free = matches!(viewer.sheet, Sheet::Closed)
        && matches!(viewer.palette, Palette::Closed)
        && matches!(viewer.context, ContextMenu::Closed);
    if !free || !matches!(viewer.stage, Stage::Pdf(_)) {
        return None;
    }
    let markup = &viewer.markup;
    let busy = markup.has_selection() || markup.picked().is_some() || markup.writing().is_some();
    match keys {
        [ShortcutKey::Escape] if busy => Some(MarkupIn::Clear),
        [ShortcutKey::Backspace] | [ShortcutKey::Delete]
            if markup.picked().is_some() && markup.writing().is_none() =>
        {
            Some(MarkupIn::Delete)
        }
        [ShortcutKey::Tab] if markup.focus().is_some() => Some(MarkupIn::Next),
        [ShortcutKey::Shift, ShortcutKey::Tab] if markup.focus().is_some() => {
            Some(MarkupIn::Previous)
        }
        _ => None,
    }
}

/// Whether `out` writes the pages of the open PDF (a page deleted, moved or turned).
fn writes_pages(out: &ViewerOut) -> bool {
    matches!(
        out,
        ViewerOut::Edit(request)
            | ViewerOut::Sheet(SheetOut::Edit(request))
            | ViewerOut::Stage(StageOut::Pdf(PdfOut::Edit(request)))
            if request.edit != Edit::Markup
    )
}

/// A page edit of a PDF with fields filled or marks made is one change with them, as in Preview:
/// the markup is written first and the page edit follows, so a save of the pages never takes the
/// person's unsaved writing with it (the file is read again after each write, and the markup
/// starts afresh on the copy that holds it). The host does the two writes one after the other,
/// each keeping the file it replaces as a version.
pub(super) fn markup_first(viewer: &Viewer, mut outs: Vec<ViewerOut>) -> Vec<ViewerOut> {
    let page = match &viewer.stage {
        Stage::Pdf(pdf) if viewer.markup.is_edited() => pdf.place().page,
        Stage::Pdf(_)
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => return outs,
    };
    let saved = outs
        .iter()
        .any(|out| matches!(out, ViewerOut::Edit(request) if request.edit == Edit::Markup));
    if let Some(at) = outs.iter().position(writes_pages).filter(|_| !saved) {
        outs.insert(
            at,
            ViewerOut::Edit(EditRequest::on_page(Edit::Markup, page)),
        );
    }
    outs
}
