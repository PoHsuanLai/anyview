//! Carrying out the stage's outputs: copying rows, the player's, a find's search and showing a
//! hit.

use super::super::{data_edit, editing};
use super::file::remember;
use super::out::Carry;
use crate::families::top_for;
use crate::io::HostRequest;
use crate::{
    FindOut, HitIndex, MediaOut, PdfOut, Stage, StageIn, StageOut, TableOut, TextIn, TextOut,
    TreeOut, TypedText, ViewerIn,
};
use anyview_core::{Edit, FindOptions};
use dioxus::prelude::*;
use ds_core::selection::RowSelection;

/// What a stage asks of the window: where the person is, and its search.
pub(super) fn staged(c: &Carry, out: &StageOut) {
    let mut shelf = c.shelf;
    if let Some(resume) = out.remembered() {
        shelf.left_at.set(resume.clone());
        remember(c, resume);
    }
    if let StageOut::Media(media) = out {
        media_out(c, media);
        return;
    }
    match out {
        StageOut::Text(TextOut::BeginEdit) => return editing::begin(c),
        StageOut::Text(TextOut::EndEdit) => return editing::end(c),
        StageOut::Text(TextOut::Save) => return editing::save(c),
        StageOut::Text(TextOut::Wrapped(wrap)) => return editing::wrapped(c, *wrap),
        StageOut::Table(
            TableOut::Set { .. }
            | TableOut::AddRow { .. }
            | TableOut::DeleteRows { .. }
            | TableOut::Session(_),
        )
        | StageOut::Tree(
            TreeOut::Set { .. }
            | TreeOut::Rename { .. }
            | TreeOut::Add(_)
            | TreeOut::Delete(_)
            | TreeOut::Session(_),
        ) => return data_edit::carry(c, out),
        StageOut::Text(
            TextOut::Remember(_) | TextOut::ScrollTo(_) | TextOut::Show(_) | TextOut::Find(_),
        )
        | StageOut::Raster(_)
        | StageOut::Pdf(_)
        | StageOut::Media(_)
        | StageOut::Table(_)
        | StageOut::Tree(_) => {}
    }
    if let StageOut::Table(TableOut::Copy(rows)) = out {
        copy_rows(c, rows);
        return;
    }
    if let StageOut::Pdf(PdfOut::Edit(request)) = out {
        c.edge.request(HostRequest::Edit(*request));
        return;
    }
    if let StageOut::Pdf(pdf) = out {
        // The pages and their search are drawn from the PDF shelf, not the text's.
        shelf.pdf.carry(pdf.clone(), find_options(c));
        return;
    }
    match out.find() {
        Some(FindOut::Search(query)) => search(c, query),
        Some(FindOut::Clear) => shelf.hits.set(None),
        Some(FindOut::ShowHit(index)) => reveal(c, *index),
        None => {}
    }
}
/// Put the picked `rows` of the table on the clipboard as text, in the order they are shown. A
/// clipboard that cannot be written leaves things as they were, as a copy in a text does.
fn copy_rows(c: &Carry, rows: &RowSelection) {
    let (Some(dispatch), Some((_, doc))) = (c.dispatch(), c.shelf.shown_now()) else {
        return;
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    let sorted = c.shelf.sorted.peek().clone();
    let order = sorted.as_ref().map(|held| held.0.as_ref());
    if let Some(text) = doc.view().copy_text(&stage, rows, order)
        && let Err(error) = ds_blitz::clipboard::write_text(&text)
    {
        log::warn!("the rows could not be copied: {error}");
        c.toasts
            .push("Couldn\u{2019}t copy the rows.".to_owned(), None);
    }
}
/// What the media stage asks: a command for the player, or a trim mark to keep for the export.
fn media_out(c: &Carry, out: &MediaOut) {
    match out {
        MediaOut::Command(command) => {
            if let Some((_, doc)) = c.shelf.shown_now()
                && let Some(line) = doc.view().line()
            {
                line.send(*command);
            }
        }
        MediaOut::Marked { edge, at } => c.shelf.media.mark(*edge, *at),
        MediaOut::Buffering(_) | MediaOut::VolumeChanged(_) | MediaOut::TracksChanged => {}
    }
}
/// How the window's find matches now: the options the person left on, which the machine keeps.
fn find_options(c: &Carry) -> FindOptions {
    c.dispatch()
        .map(|dispatch| dispatch.machine.state().peek().find_options)
        .unwrap_or_default()
}
/// Search the open file for `query`. A first frame is only the start of the file, so only the
/// full open is searched; the stage asks again when it lands.
fn search(c: &Carry, query: &TypedText) {
    let options = find_options(c);
    if editing::searches_text(c) {
        return editing::search(c, query, options);
    }
    let Some((ticket, doc)) = c.shelf.loaded.peek().clone() else {
        return;
    };
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    // What is searched is what the stage shows: the sheet of a table, the page of a Markdown file.
    let stage = dispatch.machine.state().peek().stage.clone();
    if let Some(job) = doc.view().search(&stage, ticket, query, options) {
        c.edge.submit(job);
    }
}
/// Show the hit numbered `index`: a text scrolls to its line when it is not already in the page; a
/// table or a tree, whose hits are cells and nodes, are told where to put the cursor.
fn reveal(c: &Carry, index: HitIndex) {
    let (Some(dispatch), Some(found)) = (c.dispatch(), c.shelf.hits.peek().clone()) else {
        return;
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    match &stage {
        Stage::Text(text) => {
            let Some(hit) = found.0.get(index) else {
                return;
            };
            let place = text.place();
            let page = dispatch.params().stage.text.extent.page.0;
            let top = top_for(hit.line, place.line, page);
            if top != place.line {
                dispatch.send(ViewerIn::Stage(StageIn::Text(TextIn::Scroll(top))));
            }
        }
        Stage::Table(_) | Stage::Tree(_) => {
            let Some((_, doc)) = c.shelf.shown_now() else {
                return;
            };
            let sorted = c.shelf.sorted.peek().clone();
            let order = sorted.as_ref().map(|held| held.0.as_ref());
            for input in doc.view().show_hit(&stage, &found.0, order, index) {
                dispatch.send(ViewerIn::Stage(input));
            }
        }
        Stage::NoStage | Stage::Raster(_) | Stage::Pdf(_) | Stage::Media(_) => {}
    }
}
