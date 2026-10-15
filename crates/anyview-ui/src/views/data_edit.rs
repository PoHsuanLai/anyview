//! The edits to a table's cells or a tree's nodes, as the window carries them: what the stage
//! asks of them goes to the document that holds them, and what comes back is told to the stage;
//! a save is written over the file by the host, and once it has been the file is read again, so
//! the document is the one the file now holds. The edits themselves are the document's, beside
//! what it shows (`EditDone`); the transitions are the stage's.

use super::arrive::shown_path;
use super::carry::Carry;
use super::editing::{Heard, adopt, quoted};
use super::shelf::Settle;
use crate::families::SaveBytes;
use crate::io::{HostRequest, Job, SaveEnd, TextSave};
use crate::{
    Changes, EditSessionIn, Freshness, OpenError, Stage, StageIn, StageOut, TableIn, Ticket,
    TreeIn, ViewerIn, freshness,
};
use anyview_core::{FilePath, FileStamp};
use anyview_text::EditStamp;
use dioxus::prelude::*;
use ds::stack::toast_hub::ToastAction;

fn send(c: &Carry, input: ViewerIn) {
    if let Some(dispatch) = c.dispatch() {
        dispatch.send(input);
    }
}

/// Whether the stage on screen edits a table's cells or a tree's nodes.
pub(super) fn shows_data(c: &Carry) -> bool {
    c.dispatch().is_some_and(|dispatch| {
        matches!(
            dispatch.machine.state().peek().stage,
            Stage::Table(_) | Stage::Tree(_)
        )
    })
}

/// Do what the stage asked of the edits: the document does it, the stage hears what they came
/// to, and a save is written.
pub(super) fn carry(c: &Carry, out: &StageOut) {
    let (Some(dispatch), Some((ticket, doc))) = (c.dispatch(), c.shelf.shown_now()) else {
        return;
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    let sorted = c.shelf.sorted.peek().clone();
    let order = sorted.as_ref().map(|held| held.0.as_ref());
    let done = doc.view().edit(ticket, &stage, out, order);
    for input in done.tell {
        dispatch.send(ViewerIn::Stage(input));
    }
    match done.write {
        SaveBytes::Nothing => {}
        SaveBytes::Bytes(bytes, stamp) => write(c, bytes, stamp),
        SaveBytes::Building(job) => c.edge.submit(job),
    }
}

/// Write `bytes`, the file as the edits of the state `stamp` make it, over the open file: the host
/// keeps the original and writes, and says how it ended (`saved`).
fn write(c: &Carry, bytes: Vec<u8>, stamp: EditStamp) {
    let edit = c.shelf.edit;
    let (mut settle, mut writing) = (edit.settle, edit.data_writing);
    writing.set(Some(stamp));
    settle.set(Settle::Saving);
    c.edge.request(HostRequest::SaveText(TextSave::new(bytes)));
}

/// A worker built the file a table's edits make, or could not.
pub(super) fn built(
    c: &Carry,
    ticket: Ticket,
    stamp: EditStamp,
    result: Result<Vec<u8>, OpenError>,
) {
    if c.shelf.shown_now().map(|(held, _)| held) != Some(ticket) {
        return;
    }
    match result {
        Ok(bytes) => write(c, bytes, stamp),
        Err(_) => {
            let name = quoted(c);
            c.toasts.push(format!("Couldn\u{2019}t save {name}."), None);
            let mut settle = c.shelf.edit.settle;
            settle.set(Settle::Idle);
            send(c, ViewerIn::Saved(SaveEnd::Refused));
        }
    }
}

/// The host's save ended. A written file is the state of the edits that were written: the document
/// marks it, the stage is told, the stamp the file now has is read so the write is not taken for
/// another program's, and the file is read again when it has been.
pub(super) fn saved(c: &Carry, path: &FilePath, end: SaveEnd) {
    let edit = c.shelf.edit;
    let (mut settle, writing) = (edit.settle, edit.data_writing);
    match end {
        SaveEnd::Refused => settle.set(Settle::Idle),
        SaveEnd::Written => {
            if let (Some(stamp), Some((_, doc))) = (*writing.peek(), c.shelf.shown_now()) {
                for input in doc.view().written(stamp) {
                    send(c, ViewerIn::Stage(input));
                }
            }
            settle.set(Settle::Reading);
            c.edge.submit(Job::Stat { path: path.clone() });
        }
    }
    send(c, ViewerIn::Saved(end));
}

/// The stamp the file has now, when it is the file under a table or a tree. The window's own
/// writes move the stamp the window remembers, and once the stamp of a write is read the file is
/// opened again; another program's changes are told to the person when they have changes of their
/// own, and otherwise the file is read again as for any change.
pub(super) fn stamped(
    c: &Carry,
    known: FileStamp,
    path: &FilePath,
    now: FileStamp,
    showing: &Stage,
) -> Heard {
    let mut settle = c.shelf.edit.settle;
    let settling = *settle.peek();
    match settling {
        Settle::Saving => {
            adopt(c, path, now);
            Heard::Taken
        }
        Settle::Reading => {
            adopt(c, path, now);
            settle.set(Settle::Idle);
            send(c, ViewerIn::Reload(path.clone()));
            Heard::Taken
        }
        Settle::Idle => match (freshness(known, Some(now)), showing.data_changes()) {
            (Freshness::Changed, Changes::Unsaved) => {
                adopt(c, path, now);
                changed_outside(c, showing);
                Heard::Taken
            }
            (Freshness::Changed, Changes::Saved) | (Freshness::Current, _) => Heard::NotMine,
        },
    }
}

/// The person is told that another program changed the file while they have changes of their own,
/// and may take the file as it is now in their place.
fn changed_outside(c: &Carry, showing: &Stage) {
    let name = quoted(c);
    let carry = c.clone();
    let discard = match showing {
        Stage::Table(_) => Some(StageIn::Table(TableIn::Session(EditSessionIn::Discard))),
        Stage::Tree(_) => Some(StageIn::Tree(TreeIn::Session(EditSessionIn::Discard))),
        Stage::NoStage | Stage::Raster(_) | Stage::Pdf(_) | Stage::Media(_) | Stage::Text(_) => {
            None
        }
    };
    c.toasts.push_action(
        format!("{name} was changed by another application"),
        ToastAction::new("Reload"),
        EventHandler::new(move |()| {
            if let Some(input) = discard.clone() {
                send(&carry, ViewerIn::Stage(input));
            }
            if let Some(path) = shown_path(&carry) {
                send(&carry, ViewerIn::Reload(path));
            }
        }),
    );
}
