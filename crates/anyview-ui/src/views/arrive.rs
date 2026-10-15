//! What a worker made, as input to the machines. Every result carries the ticket of the file it
//! was asked for, and one for a file the person has left is dropped here or by the machine.

use super::carry::Carry;
use super::data_edit;
use super::editing;
use super::session::Probe;
use crate::families::{FoundHits, Held, SortedOrder};
use crate::io::{Done, HostRequest, Job, NotOpened, Notice, SizeBasis};
use crate::io::{NaturalSize, cause_chain};
use crate::{
    Allowance, EditOffer, FileAccess, Freshness, HelperEnd, HelperPhase, HitCount, HitIndex,
    LoadIn, MarkupIn, NavigateIn, Panel, PictureEditIn, PictureEditing, Presentation, Sheet,
    SheetIn, Stage, StageIn, TableIn, TextIn, TextStage, Ticket, TreeIn, TypedText, VersionList,
    ViewerIn, freshness,
};
use anyview_core::{FilePath, FindOptions, Helper, LineIndex, PixelLen, PixelSize, Resume};
use dioxus::prelude::*;
use ds::stack::toast_hub::ToastAction;
use std::sync::Arc;

/// Feed the result `done` to the window's machines.
pub(super) fn arrived(done: Done, c: &Carry) {
    match done {
        Done::Probed { ticket, result } => probed(c, ticket, result),
        Done::Peeked { ticket, result } => peeked(c, ticket, result),
        Done::Opened { ticket, result } => opened(c, ticket, result),
        Done::Lines { ticket, result } => {
            let mut lines = c.shelf.lines;
            let Some((held, _)) = c.shelf.shown_now() else {
                return;
            };
            if held != ticket {
                return;
            }
            match result {
                Ok(window) => lines.set(Some(Held(Arc::new(window)))),
                Err(error) => couldnt(c, "read", &error),
            }
        }
        Done::Sorted { ticket, order } => sorted(c, ticket, order),
        Done::EditText { ticket, result } => editing::read(c, ticket, result),
        Done::DataBytes {
            ticket,
            stamp,
            result,
        } => data_edit::built(c, ticket, stamp, result),
        Done::Saved { path, end } => editing::saved(c, &path, end),
        Done::Found {
            ticket,
            query,
            options,
            result,
        } => match result {
            Ok(hits) => found(c, ticket, (query, options), hits),
            Err(error) => {
                if c.shelf.loaded.peek().as_ref().map(|(held, _)| *held) == Some(ticket) {
                    couldnt(c, "search", &error);
                }
            }
        },
        Done::Pdf { ticket, answer } => pdf_arrived(c, ticket, answer),
        Done::Media { ticket } => media_arrived(c, ticket),
        Done::Preloaded { path, loaded } => {
            let mut preloads = c.shelf.preloads;
            preloads.write().arrived(&path, loaded);
        }
        Done::Stamped { path, stamp } => stamped(c, &path, stamp),
        Done::Folder { path, result } => folder(c, &path, result),
        Done::Versions { path, rows } => {
            if shown_path(c).as_ref() == Some(&path) {
                send(
                    c,
                    ViewerIn::Sheet(SheetIn::OpenRevert(VersionList::new(rows))),
                );
            }
        }
        Done::Kept => {}
        Done::Notice(notice) => noticed(c, notice),
        Done::Helped(helper, end) => send(c, ViewerIn::Sheet(SheetIn::HelperEnded(helper, end))),
        Done::Available(helper) => available(c, helper),
        Done::Chosen { files } => send(c, ViewerIn::Chosen(files)),
        Done::Moved { to } => send(c, ViewerIn::Reload(to)),
        Done::Changed { path } => {
            if shown_path(c).as_ref() == Some(&path) {
                c.edge.submit(Job::Stat { path });
            }
        }
    }
}

/// The order a worker gave a sheet is kept only while it is the order the person still wants: the
/// file is the one shown and the stage is sorted by this sheet and column. An answer to a sort that
/// a newer press has replaced is dropped.
fn sorted(c: &Carry, ticket: Ticket, order: Option<SortedOrder>) {
    let (Some(order), Some(dispatch)) = (order, c.dispatch()) else {
        return;
    };
    let shown = c.shelf.shown_now().is_some_and(|(held, _)| held == ticket);
    let wanted = match &dispatch.machine.state().peek().stage {
        Stage::Table(stage) => stage.sheet() == order.sheet && stage.sort() == Some(order.sort),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => false,
    };
    if shown && wanted {
        let mut kept = c.shelf.sorted;
        kept.set(Some(Held(Arc::new(order))));
    }
}

/// A tool a plugin runs is on the machine now, installed some other way. What waited for it opens
/// again: the sheet that asked for it closes (the install it offered is done), and the file that
/// lacked it is probed afresh. An install this window is running itself ends in its own answer.
fn available(c: &Carry, helper: Helper) {
    let mut preloads = c.shelf.preloads;
    preloads.write().forget_lacking(helper);
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let sheet = dispatch.machine.state().peek().sheet.clone();
    match sheet {
        Sheet::Helper {
            helper: asked,
            phase,
        } if asked == helper => match phase {
            HelperPhase::Installing => {}
            HelperPhase::Ask
            | HelperPhase::Failed(_)
            | HelperPhase::NotFound(_)
            | HelperPhase::Unsupported(_) => {
                dispatch.send(ViewerIn::Sheet(SheetIn::HelperEnded(
                    helper,
                    HelperEnd::Installed,
                )));
            }
        },
        Sheet::Closed
        | Sheet::Export { .. }
        | Sheet::Unavailable { .. }
        | Sheet::ConfirmTrash
        | Sheet::ConfirmEdit { .. }
        | Sheet::Rename { .. }
        | Sheet::SaveCopy { .. }
        | Sheet::Revert { .. }
        | Sheet::NoVersions
        | Sheet::Helper { .. }
        | Sheet::Picture(_)
        | Sheet::Unsaved(_)
        | Sheet::ConfirmReplace => {
            let lacking = c
                .shelf
                .shown_now()
                .is_some_and(|(_, doc)| doc.view().lacks() == Some(helper));
            if let (true, Some(path)) = (lacking, shown_path(c)) {
                dispatch.send(ViewerIn::Reload(path));
            }
        }
    }
}

/// The host says how a task ended: a toast, with "Show in Folder" when it names a file.
fn noticed(c: &Carry, notice: Notice) {
    let Notice { text, reveal } = notice;
    match reveal {
        Some(file) => {
            let edge = c.edge.clone();
            c.toasts.push_action(
                text,
                ToastAction::new("Show in Folder"),
                EventHandler::new(move |()| edge.request(HostRequest::Reveal(file.clone()))),
            );
        }
        None => c.toasts.push(text, None),
    }
}

/// The player of `ticket` has news. A wake that comes before the document has landed is not lost:
/// the line keeps what it has, and `landed` drains it.
fn media_arrived(c: &Carry, ticket: Ticket) {
    if c.shelf.shown_now().map(|(held, _)| held) == Some(ticket) {
        drain_media(c);
    }
}

/// Read what the player reported: the shelf keeps it for the views, the stage machine hears the
/// part that moves it, and the place the person is at is kept for next time when it has moved.
pub(super) fn drain_media(c: &Carry) {
    let Some((_, doc)) = c.shelf.shown_now() else {
        return;
    };
    let Some(line) = doc.view().line() else {
        return;
    };
    let notices = line.drain();
    if notices.is_empty() {
        return;
    }
    c.shelf.media.apply(&notices);
    for notice in &notices {
        if let Some(input) = stage_input_of(notice) {
            send(c, ViewerIn::Stage(StageIn::Media(input)));
        }
        if let Some(walk) = walk_of(notice) {
            send(c, ViewerIn::Navigate(walk));
        }
    }
    remember_place(c);
}

/// The input the stage machine takes from a piece of the player's news, if it takes one.
fn stage_input_of(notice: &crate::MediaNotice) -> Option<crate::MediaIn> {
    match notice {
        crate::MediaNotice::Player(event) => Some(crate::MediaIn::Player(*event)),
        crate::MediaNotice::Position(at) => Some(crate::MediaIn::Position(*at)),
        crate::MediaNotice::Failed(error) => Some(crate::MediaIn::Failed(*error)),
        crate::MediaNotice::Tracks(_)
        | crate::MediaNotice::Chapters(_)
        | crate::MediaNotice::Speed(_)
        | crate::MediaNotice::Abilities(_)
        | crate::MediaNotice::Picture(_)
        | crate::MediaNotice::Next
        | crate::MediaNotice::Previous => None,
    }
}

/// The walk along the folder's files that the player's news asks for, if it asks.
fn walk_of(notice: &crate::MediaNotice) -> Option<crate::NavigateIn> {
    match notice {
        crate::MediaNotice::Next => Some(crate::NavigateIn::Next),
        crate::MediaNotice::Previous => Some(crate::NavigateIn::Previous),
        crate::MediaNotice::Player(_)
        | crate::MediaNotice::Position(_)
        | crate::MediaNotice::Tracks(_)
        | crate::MediaNotice::Chapters(_)
        | crate::MediaNotice::Speed(_)
        | crate::MediaNotice::Abilities(_)
        | crate::MediaNotice::Picture(_)
        | crate::MediaNotice::Failed(_) => None,
    }
}

/// Keep where the person is in the recording when it is not where it was last kept (to the
/// second, or in its settings), while the recording is open and not still opening, and once the
/// player has said where playback is: until then it has not applied the place the person left.
fn remember_place(c: &Carry) {
    let mut shelf = c.shelf;
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let opening = matches!(
        dispatch.machine.state().peek().stage,
        Stage::Media(crate::MediaStage::Opening)
    );
    if opening {
        return;
    }
    let Some(place) = shelf.media.place() else {
        return;
    };
    let kept = crate::MediaPlace::of(&shelf.left_at.peek());
    if kept.is_none_or(|kept| !kept.same_as(place)) {
        shelf.left_at.set(place.resume());
        c.edge.request(crate::HostRequest::Remember(place.resume()));
    }
}

/// What a PDF worker made: the shelf takes it in, and the stage machine hears of it when it must.
/// An answer for a file the person left is dropped by its ticket.
fn pdf_arrived(c: &Carry, ticket: Ticket, answer: crate::PdfAnswer) {
    if c.shelf.probe.peek().ticket() != Some(ticket) {
        return;
    }
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    if let Some(input) = c.shelf.pdf.arrived(answer, &stage) {
        dispatch.send(ViewerIn::Stage(input));
    }
}

/// The file on screen.
pub(super) fn shown_path(c: &Carry) -> Option<FilePath> {
    c.shelf
        .probe
        .peek()
        .found()
        .map(|probed| probed.source.path().clone())
}

fn send(c: &Carry, input: ViewerIn) {
    if let Some(dispatch) = c.dispatch() {
        dispatch.send(input);
    }
}

/// A file that would not load is still a different file: it sizes the window as any new file
/// with no natural size does (the default window), rather than leaving the last file's size.
fn failed(c: &Carry, ticket: Ticket, error: &crate::OpenError) {
    log::warn!(
        "{} could not be opened: {}",
        wanted_name(c),
        cause_chain(error)
    );
    size_window(c, ticket, SizeBasis::Default);
    let reason = error.failure();
    if let Some(file) = c.shelf.wanted.peek().clone() {
        c.edge
            .request(HostRequest::NotOpened(NotOpened { file, reason }));
    }
    send(c, ViewerIn::Load(LoadIn::Failed { ticket, reason }));
}

/// The name of the file the window was asked for, for a log line.
fn wanted_name(c: &Carry) -> String {
    c.shelf
        .wanted
        .peek()
        .as_ref()
        .map_or_else(String::new, |file| file.as_path().display().to_string())
}

/// Work on the file on screen failed after it had opened: say so, as the saves do, rather than
/// leave the person with an empty answer or a blank page.
fn couldnt(c: &Carry, what: &str, error: &crate::OpenError) {
    let name = shown_path(c).and_then(|path| path.file_name().map(|name| name.as_str().to_owned()));
    log::warn!(
        "could not {what} {}: {}",
        name.as_deref().unwrap_or("the file"),
        cause_chain(error)
    );
    c.toasts.push(
        format!(
            "Couldn\u{2019}t {what} {}.",
            name.as_deref().unwrap_or("the file")
        ),
        None,
    );
}

fn probed(c: &Carry, ticket: Ticket, result: Result<crate::Opened, crate::OpenError>) {
    let mut probe = c.shelf.probe;
    let now = probe.peek().clone();
    if now.ticket() != Some(ticket) {
        return;
    }
    match result {
        Ok(found) => {
            // A reload keeps the stage, and so the place the person is at: what the store
            // remembered is older.
            let found = match now {
                Probe::Reprobing(..) => crate::Opened {
                    resume: Resume::Nothing,
                    ..found
                },
                Probe::Idle | Probe::Pending(_) | Probe::Arrived(..) | Probe::Announced(..) => {
                    found
                }
            };
            probe.set(Probe::Arrived(ticket, found));
        }
        Err(error) => failed(c, ticket, &error),
    }
}

fn peeked(c: &Carry, ticket: Ticket, result: Result<Option<crate::LoadedDoc>, crate::OpenError>) {
    let mut peeked = c.shelf.peeked;
    let current = c.shelf.probe.peek().ticket() == Some(ticket);
    let landed = c.shelf.loaded.peek().is_some();
    match result {
        Ok(Some(doc)) if current && !landed => {
            peeked.set(Some((ticket, doc)));
            send(c, ViewerIn::Load(LoadIn::Peeked { ticket }));
        }
        Ok(Some(_)) => {}
        Ok(None) => send(c, ViewerIn::Load(LoadIn::PeekFailed { ticket })),
        Err(error) => {
            log::warn!(
                "{} could not be previewed: {}",
                wanted_name(c),
                cause_chain(&error)
            );
            send(c, ViewerIn::Load(LoadIn::PeekFailed { ticket }));
        }
    }
}

fn opened(c: &Carry, ticket: Ticket, result: Result<crate::LoadedDoc, crate::OpenError>) {
    let (mut loaded, mut peeked, mut lines, mut hits) =
        (c.shelf.loaded, c.shelf.peeked, c.shelf.lines, c.shelf.hits);
    if c.shelf.probe.peek().ticket() != Some(ticket) {
        return;
    }
    match result {
        Ok(doc) => {
            loaded.set(Some((ticket, doc.clone())));
            peeked.set(None);
            lines.set(None);
            hits.set(None);
            landed(c, ticket, &doc);
        }
        Err(error) => failed(c, ticket, &error),
    }
}

/// The full open of `ticket` is on screen: the load is ready, and the stage is told what the
/// document says of itself (an animation moves, a find already up asks again).
pub(super) fn landed(c: &Carry, ticket: Ticket, doc: &crate::LoadedDoc) {
    send(c, ViewerIn::Load(LoadIn::Opened { ticket }));
    // Whatever was done to the last picture is gone with it (or was just saved into this one).
    send(c, ViewerIn::Picture(picture_landed(c, doc)));
    if let Some(landing) = markup_landed(c, doc) {
        send(c, ViewerIn::Markup(landing));
    }
    // The panel stays open from file to file; its tab stays too when the new file has it.
    send(c, ViewerIn::Panel(crate::PanelIn::TabsChanged));
    size_window(
        c,
        ticket,
        doc.view().natural().map_or(SizeBasis::Header, |natural| {
            SizeBasis::Natural(with_panel(c, natural))
        }),
    );
    for input in told_of(c, doc) {
        send(c, ViewerIn::Stage(input));
    }
    // A player that started before the document landed may have woken the window already.
    drain_media(c);
}

/// `natural` widened by the panel's width when a window that sizes itself shows the panel beside
/// the content (a PDF opens it, as Preview opens its sidebar): the window then opens with the
/// whole page showing, not squeezed by the panel. A pane is its host's region and is not sized
/// this way, and nor is a picture, whose size is in device pixels.
fn with_panel(c: &Carry, natural: NaturalSize) -> NaturalSize {
    let NaturalSize::Points(page) = natural else {
        return natural;
    };
    let beside = c.dispatch().is_some_and(|dispatch| {
        let state = dispatch.machine.state();
        let state = state.peek();
        state.presentation != Presentation::Pane && matches!(state.panel, Panel::Shown { .. })
    });
    if !beside {
        return natural;
    }
    NaturalSize::Points(PixelSize {
        width: PixelLen(page.width.0.saturating_add(super::panel::WIDTH)),
        ..page
    })
}

/// What the picture edits are told when `doc` lands: its size, and whether it can be saved with
/// changes (a picture whose format has a writer, in a file that takes a save in place).
fn picture_landed(c: &Carry, doc: &crate::LoadedDoc) -> PictureEditIn {
    let on_a_picture = c
        .dispatch()
        .is_some_and(|dispatch| matches!(dispatch.machine.state().peek().stage, Stage::Raster(_)));
    let writable = c
        .shelf
        .probe
        .peek()
        .found()
        .is_some_and(|probed| probed.access == FileAccess::Writable);
    let size = match doc.view().natural() {
        Some(NaturalSize::Pixels(size)) if on_a_picture => Some(size),
        Some(NaturalSize::Pixels(_) | NaturalSize::Points(_) | NaturalSize::Compact(_)) | None => {
            None
        }
    };
    let editing = if on_a_picture && writable && doc.view().edit_offer() != EditOffer::Withheld {
        PictureEditing::Allowed
    } else {
        PictureEditing::Locked
    };
    PictureEditIn::Landed { size, editing }
}

/// What the markup is told when `doc` lands, if it is a PDF: where its fields are, and what it lets
/// the person write on it. A pane changes nothing in the file, and neither does a window on a file
/// that refuses a save, so nothing is offered there.
fn markup_landed(c: &Carry, doc: &crate::LoadedDoc) -> Option<MarkupIn> {
    let MarkupIn::Landed {
        widgets,
        filling,
        marking,
        marks,
    } = doc.view().markup()?
    else {
        return None;
    };
    let pane = c
        .dispatch()
        .is_some_and(|dispatch| dispatch.machine.state().peek().presentation == Presentation::Pane);
    let writable = c
        .shelf
        .probe
        .peek()
        .found()
        .is_some_and(|probed| probed.access == FileAccess::Writable);
    let (filling, marking) = if pane || !writable {
        (Allowance::Withheld, Allowance::Withheld)
    } else {
        (filling, marking)
    };
    Some(MarkupIn::Landed {
        widgets,
        filling,
        marking,
        marks,
    })
}

/// A different file has loaded: tell the host how big its content naturally is, once, so it can
/// size the window to the file as it did for the first. The same file loaded again (a reload) never
/// does, and nor does a file the person has already left.
fn size_window(c: &Carry, ticket: Ticket, basis: SizeBasis) {
    let mut sizing = c.shelf.sizing;
    if *sizing.peek() != Some(ticket) {
        return;
    }
    sizing.set(None);
    c.edge.request(HostRequest::SizeWindow(basis));
}

/// The inputs the stage is given now that `doc` is on screen.
fn told_of(c: &Carry, doc: &crate::LoadedDoc) -> Vec<StageIn> {
    let Some(dispatch) = c.dispatch() else {
        return Vec::new();
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    let left_at = c
        .shelf
        .probe
        .peek()
        .found()
        .map_or(Resume::Nothing, |probed| probed.resume.clone());
    doc.view().arrived(&stage, &left_at)
}

/// How a stage is told its search answered.
type Answer = fn(TypedText, HitCount, HitIndex) -> StageIn;

fn text_results(query: TypedText, count: HitCount, nearest: HitIndex) -> StageIn {
    StageIn::Text(TextIn::Results {
        query,
        count,
        nearest,
    })
}

fn table_results(query: TypedText, count: HitCount, nearest: HitIndex) -> StageIn {
    StageIn::Table(TableIn::Results {
        query,
        count,
        nearest,
    })
}

fn tree_results(query: TypedText, count: HitCount, nearest: HitIndex) -> StageIn {
    StageIn::Tree(TreeIn::Results {
        query,
        count,
        nearest,
    })
}

/// The find the stage has up, and what the answer to it needs: the text searched for, the line the
/// reader is at (the hit nearest it is the one to show first), and how the stage hears the answer.
/// `None` when no find is up, or when `hits` are of something the stage no longer shows (a table's
/// hits are of one sheet).
fn asking(stage: &Stage, hits: &FoundHits) -> Option<(TypedText, LineIndex, Answer)> {
    match stage {
        Stage::Text(TextStage::Finding { query, place, .. }) => {
            Some((query.clone(), place.line, text_results as Answer))
        }
        Stage::Table(table) => {
            let find = table.find()?;
            // The rows a cursor is on are where the sort put them: only the file's own order
            // says which hit a reader is near.
            let row = table
                .row()
                .filter(|_| table.sort().is_none())
                .map_or(0, |row| row.0);
            hits.sheet()
                .is_none_or(|sheet| sheet == table.sheet())
                .then(|| (find.query.clone(), LineIndex(row), table_results as Answer))
        }
        Stage::Tree(tree) => {
            let find = tree.find()?;
            Some((find.query.clone(), LineIndex(0), tree_results as Answer))
        }
        Stage::Text(TextStage::Reading { .. } | TextStage::Editing { .. })
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_) => None,
    }
}

/// A search answered: the hits are kept for the view, and the stage is told how many there are
/// and which is nearest the reader.
fn found(c: &Carry, ticket: Ticket, asked: (TypedText, FindOptions), hits: FoundHits) {
    let (query, options) = asked;
    let mut shelf = c.shelf;
    if shelf.loaded.peek().as_ref().map(|(held, _)| *held) != Some(ticket) {
        return;
    }
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let (stage, current) = {
        let signal = dispatch.machine.state();
        let state = signal.peek();
        (state.stage.clone(), state.find_options)
    };
    let Some((up, at, answer)) = asking(&stage, &hits) else {
        return;
    };
    // An answer for another text, or for options since switched, is no answer to the find now up.
    if up != query || current != options {
        return;
    }
    let (count, nearest) = (hits.count(), hits.nearest(at));
    shelf.hits.set(Some(Held(Arc::new(hits))));
    dispatch.send(ViewerIn::Stage(answer(query, count, nearest)));
}

/// The stamp of the file on screen as the disk has it now: a file that differs is opened again.
fn stamped(c: &Carry, path: &FilePath, stamp: Option<anyview_core::FileStamp>) {
    let Some(probed) = c.shelf.probe.peek().found().cloned() else {
        return;
    };
    if probed.source.path() != path {
        return;
    }
    // A text being edited keeps its place when its file changes: the edit decides what to do.
    if editing::stamped(c, probed.source.stamp(), path, stamp) == editing::Heard::Taken {
        return;
    }
    match freshness(probed.source.stamp(), stamp) {
        Freshness::Changed => send(c, ViewerIn::Reload(path.clone())),
        Freshness::Current => {}
    }
}

/// The folder of a dropped file, as the list the arrows walk, if the file is still the one asked
/// about.
fn folder(c: &Carry, path: &FilePath, result: Result<anyview_core::Sequence, crate::OpenError>) {
    let mut asked = c.shelf.folder;
    if asked.peek().as_ref() != Some(path) {
        return;
    }
    asked.set(None);
    if let Ok(sequence) = result {
        send(c, ViewerIn::Navigate(NavigateIn::Start(sequence)));
    }
}
