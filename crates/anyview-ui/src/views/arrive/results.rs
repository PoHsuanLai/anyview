//! A worker's result routed to the machine that wants it: which kind of result it is, and the
//! small answers to a stamp, a folder, a sort, a missing helper or a notice.

use super::super::carry::Carry;
use super::super::{data_edit, editing};
use super::find::found;
use super::media::{media_arrived, pdf_arrived};
use super::open::{couldnt, opened, peeked, probed};
use crate::families::{Held, SortedOrder};
use crate::io::{Done, HostRequest, Job, Notice};
use crate::{
    Freshness, HelperEnd, HelperPhase, NavigateIn, Sheet, SheetIn, Stage, Ticket, VersionList,
    ViewerIn, freshness,
};
use anyview_core::{FilePath, Helper};
use dioxus::prelude::*;
use ds::stack::toast_hub::ToastAction;
use std::sync::Arc;

/// Feed the result `done` to the window's machines.
pub(in crate::views) fn arrived(done: Done, c: &Carry) {
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
            | HelperPhase::Unsupported { .. } => {
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

/// The file on screen.
pub(in crate::views) fn shown_path(c: &Carry) -> Option<FilePath> {
    c.shelf
        .probe
        .peek()
        .found()
        .map(|probed| probed.source.path().clone())
}

pub(super) fn send(c: &Carry, input: ViewerIn) {
    if let Some(dispatch) = c.dispatch() {
        dispatch.send(input);
    }
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
