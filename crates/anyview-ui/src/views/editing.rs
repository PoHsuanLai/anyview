//! The text being edited, as the window carries it: reading the file for it, saving it, searching
//! it, and noticing that the file under it changed. The text itself is the shelf's `Session`; the
//! machines hear only whether it has unsaved changes and whether the file changed on disk.

use super::arrive::shown_path;
use super::carry::Carry;
use super::shelf::{Doc, Settle};
use crate::families::{Held, found_in};
use crate::io::{HostRequest, Job, SaveEnd, TextSave};
use crate::stage::WrapChoices;
use crate::{
    Changes, Freshness, Outside, Stage, StageIn, TextIn, Ticket, TypedText, ViewerIn, freshness,
};
use anyview_core::{FilePath, FileStamp, LineIndex, Source};
use anyview_text::{EditRefusal, EditText, Needle, Session};
use dioxus::prelude::*;
use ds::stack::toast_hub::ToastAction;
use std::sync::Arc;

/// What the window did with a change of the stamp of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Heard {
    /// The edited text took it in.
    Taken,
    /// No text is being edited: the file is reloaded as for any change.
    NotMine,
}

fn send(c: &Carry, input: ViewerIn) {
    if let Some(dispatch) = c.dispatch() {
        dispatch.send(input);
    }
}

fn tell(c: &Carry, input: TextIn) {
    send(c, ViewerIn::Stage(StageIn::Text(input)));
}

/// The file's name between quotes, for a sentence.
fn quoted(c: &Carry) -> String {
    shown_path(c)
        .and_then(|path| path.file_name().map(|name| name.as_str().to_owned()))
        .map_or_else(
            || "the file".to_owned(),
            |name| format!("\u{201c}{name}\u{201d}"),
        )
}

/// Editing began: the file is read whole on a worker, and the text appears when it arrives.
pub(super) fn begin(c: &Carry) {
    let Some((ticket, doc)) = c.shelf.shown_now() else {
        return;
    };
    if let Some(job) = doc.view().edit_read(ticket) {
        c.edge.submit(job);
    }
}

/// The worker read the file for editing, or could not.
pub(super) fn read(c: &Carry, ticket: Ticket, result: Result<EditText, EditRefusal>) {
    if c.shelf.probe.peek().ticket() != Some(ticket) {
        return;
    }
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    // The person may have left the editing while the file was read.
    if dispatch.machine.state().peek().stage.edited().is_none() {
        return;
    }
    match result {
        Ok(text) => {
            let mut session = c.shelf.edit.session;
            session.set(Some(Session::open(text)));
            // A find that is up searches the text now it is there.
            tell(c, TextIn::Edited(Changes::Saved));
        }
        Err(refusal) => {
            let name = quoted(c);
            c.toasts.push(refusal_words(&name, refusal), None);
            tell(c, TextIn::Done);
        }
    }
}

/// Why a file is not edited, in the person's words.
fn refusal_words(name: &str, refusal: EditRefusal) -> String {
    match refusal {
        EditRefusal::NotUtf8 => {
            format!("{name} isn\u{2019}t saved as UTF-8 text, so it can\u{2019}t be edited here.")
        }
        EditRefusal::TooLarge => format!("{name} is too large to edit."),
        EditRefusal::Unreadable(_) => format!("Couldn\u{2019}t open {name} for editing."),
    }
}

/// The editing ended: the text is let go. The document still holds the file as it was opened, so a
/// file that was written meanwhile is opened again.
pub(super) fn end(c: &Carry) {
    let edit = c.shelf.edit;
    let (mut session, mut settle, mut doc, mut writing) =
        (edit.session, edit.settle, edit.doc, edit.writing);
    session.set(None);
    settle.set(Settle::Idle);
    writing.set(None);
    let stale = *doc.peek() == Doc::Stale;
    doc.set(Doc::Current);
    if stale && let Some(path) = shown_path(c) {
        send(c, ViewerIn::Reload(path));
    }
}

/// A different file is being opened: nothing of the last one's editing is left.
pub(super) fn forget(c: &Carry) {
    let edit = c.shelf.edit;
    let (mut session, mut settle, mut doc, mut writing) =
        (edit.session, edit.settle, edit.doc, edit.writing);
    session.set(None);
    settle.set(Settle::Idle);
    doc.set(Doc::Current);
    writing.set(None);
}

/// Write the edited text over the file: the host keeps the original and writes, and says how it
/// ended (`saved`).
pub(super) fn save(c: &Carry) {
    let edit = c.shelf.edit;
    let (mut settle, mut writing) = (edit.settle, edit.writing);
    let written = {
        let held = edit.session.peek();
        held.as_ref()
            .map(|text| (text.file_bytes(), text.revision()))
    };
    let Some((bytes, revision)) = written else {
        return;
    };
    writing.set(Some(revision));
    settle.set(Settle::Saving);
    c.edge.request(HostRequest::SaveText(TextSave::new(bytes)));
}

/// The host's save ended. A written text is clean as of the revision that was written (what was
/// typed meanwhile stays unsaved), the stamp the file now has is read so the write is not taken
/// for another program's, and what waited for the save goes ahead.
pub(super) fn saved(c: &Carry, path: &FilePath, end: SaveEnd) {
    if shown_path(c).as_ref() != Some(path) {
        return;
    }
    let edit = c.shelf.edit;
    let (mut session, mut settle, mut doc) = (edit.session, edit.settle, edit.doc);
    match end {
        SaveEnd::Refused => settle.set(Settle::Idle),
        SaveEnd::Written => {
            let revision = *edit.writing.peek();
            if let (Some(revision), Some(text)) = (revision, session.write().as_mut()) {
                text.mark_saved(revision);
            }
            settle.set(Settle::Reading);
            doc.set(Doc::Stale);
            c.edge.submit(Job::Stat { path: path.clone() });
            let changes = session.peek().as_ref().map_or(Changes::Saved, |text| {
                if text.is_modified() {
                    Changes::Unsaved
                } else {
                    Changes::Saved
                }
            });
            tell(c, TextIn::Edited(changes));
            tell(c, TextIn::Disk(Outside::Unchanged));
        }
    }
    send(c, ViewerIn::Saved(end));
}

/// The stamp the file has now, when it is the file under an edited text. The window's own writes
/// move the stamp the window remembers; another program's changes are told to the person, and a
/// file whose text has no changes of the person's is read again.
pub(super) fn stamped(
    c: &Carry,
    known: FileStamp,
    path: &FilePath,
    stamp: Option<FileStamp>,
) -> Heard {
    let Some(dispatch) = c.dispatch() else {
        return Heard::NotMine;
    };
    let showing: Stage = dispatch.machine.state().peek().stage.clone();
    let (Some(edited), Some(now)) = (showing.edited(), stamp) else {
        return Heard::NotMine;
    };
    let edit = c.shelf.edit;
    let mut settle = edit.settle;
    let settling = *settle.peek();
    match settling {
        Settle::Saving => adopt(c, path, now),
        Settle::Reading => {
            adopt(c, path, now);
            settle.set(Settle::Idle);
        }
        Settle::Idle => match freshness(known, Some(now)) {
            Freshness::Current => {}
            Freshness::Changed => {
                adopt(c, path, now);
                let mut doc = edit.doc;
                doc.set(Doc::Stale);
                match edited.changes {
                    Changes::Saved => begin(c),
                    Changes::Unsaved => {
                        tell(c, TextIn::Disk(Outside::Changed));
                        changed_outside(c);
                    }
                }
            }
        },
    }
    Heard::Taken
}

/// The person is told that another program changed the file while they have changes of their
/// own, and may take the file's text instead.
fn changed_outside(c: &Carry) {
    let name = quoted(c);
    let carry = c.clone();
    c.toasts.push_action(
        format!("{name} was changed by another application"),
        ToastAction::new("Reload"),
        EventHandler::new(move |()| reload(&carry)),
    );
}

/// Take the file's text in place of the person's changes.
fn reload(c: &Carry) {
    tell(c, TextIn::Edited(Changes::Saved));
    tell(c, TextIn::Disk(Outside::Unchanged));
    begin(c);
}

/// `now` is the stamp the window knows the file by from here on.
fn adopt(c: &Carry, path: &FilePath, now: FileStamp) {
    let mut probe = c.shelf.probe;
    let with = |probed: crate::Opened| crate::Opened {
        source: Source::new(path.clone(), now),
        ..probed
    };
    let next = match probe.peek().clone() {
        super::session::Probe::Announced(ticket, probed) => {
            super::session::Probe::Announced(ticket, with(probed))
        }
        super::session::Probe::Arrived(ticket, probed) => {
            super::session::Probe::Arrived(ticket, with(probed))
        }
        super::session::Probe::Reprobing(ticket, probed) => {
            super::session::Probe::Reprobing(ticket, with(probed))
        }
        super::session::Probe::Idle | super::session::Probe::Pending(_) => return,
    };
    probe.set(next);
}

/// Whether a search is over the edited text and not over the file's lines.
pub(super) fn searches_text(c: &Carry) -> bool {
    c.shelf.edit.session.peek().is_some()
        && c.dispatch()
            .is_some_and(|dispatch| dispatch.machine.state().peek().stage.edited().is_some())
}

/// Search the edited text for `query` as it is now, and tell the stage what was found.
pub(super) fn search(c: &Carry, query: &TypedText) {
    let Some(needle) = Needle::new(query.as_str()) else {
        return;
    };
    let mut hits = c.shelf.hits;
    let found = {
        let held = c.shelf.edit.session.peek();
        held.as_ref().map(|text| {
            let line = u32::try_from(text.place(text.caret()).0).unwrap_or(u32::MAX);
            (found_in(text, &needle), LineIndex(line))
        })
    };
    let Some((found, near)) = found else {
        return;
    };
    let (count, nearest) = (found.count(), found.nearest(near));
    hits.set(Some(Held(Arc::new(found))));
    tell(
        c,
        TextIn::Results {
            query: query.clone(),
            count,
            nearest,
        },
    );
}

/// How the person last chose to wrap `kind`, kept for the next file of it.
pub(super) fn wrapped(c: &Carry, wrap: crate::Wrap) {
    let Some(kind) = c
        .shelf
        .probe
        .peek()
        .found()
        .map(|probed| probed.sniffed.kind())
    else {
        return;
    };
    let mut wraps = c.shelf.wraps;
    let next: WrapChoices = wraps.peek().chose(kind, wrap);
    wraps.set(next);
}
