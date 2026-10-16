//! Carrying out the outputs about the open file: opening it, reloading it, leaving it and what
//! is kept of it, and preloading its neighbours.

use super::super::editing;
use super::super::shelf::Doc;
use super::out::Carry;
use crate::Ticket;
use crate::families::Leaving;
use crate::io::{HostRequest, Job, Preloaded};
use anyview_core::{FilePath, Neighbours, Resume};
use dioxus::prelude::*;
use ds::motion::detail::operation::{Operation, PendingToken};

/// A new file is wanted: the one on screen is kept as it was left, and the new one comes from the
/// files opened ahead when it is among them (and is checked against the disk, since it may have
/// changed since), otherwise from a probe.
pub(super) fn opened(c: &Carry, ticket: Ticket, path: FilePath) {
    let mut shelf = c.shelf;
    remember_on_leaving(c);
    keep_the_one_left(c);
    shelf.wanted.set(Some(path.clone()));
    shelf.sizing.set(Some(ticket));
    shelf.loaded.set(None);
    shelf.peeked.set(None);
    shelf.lines.set(None);
    shelf.sorted.set(None);
    shelf.hits.set(None);
    shelf.pdf.reset();
    shelf.media.reset();
    shelf.left_at.set(Resume::Nothing);
    editing::forget(c);
    shelf
        .operation
        .set(Operation::Running(PendingToken::start()));
    let ahead = shelf.preloads.write().take(&path);
    match ahead {
        Some(Preloaded { probed, doc }) => {
            shelf.loaded.set(Some((ticket, doc)));
            shelf
                .probe
                .set(super::super::session::Probe::Arrived(ticket, probed));
            c.edge.submit(Job::Stat { path });
        }
        None => {
            shelf
                .probe
                .set(super::super::session::Probe::Pending(ticket));
            c.edge.submit(Job::Probe { ticket, path });
        }
    }
}
/// The place the person last settled at is said once more as they leave the file or the window, so
/// that the host has the final one whatever it did with the others (the host knows which file from
/// the `Opened` request it was sent, and the next file's has not come yet).
pub(super) fn remember_on_leaving(c: &Carry) {
    let left = c.shelf.left_at.peek().clone();
    match left {
        Resume::Nothing => {}
        placed @ (Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Media { .. }
        | Resume::Text { .. }
        | Resume::Book { .. }) => remember(c, &placed),
    }
}
/// The open file, held as it was left: its document and where the person was in it, so coming
/// back to it needs no decode and no search for the place.
fn keep_the_one_left(c: &Carry) {
    let mut shelf = c.shelf;
    // A file written while it was edited is not what its document holds: it is opened afresh.
    if *shelf.edit.doc.peek() == Doc::Stale {
        return;
    }
    let (Some((_, doc)), Some(probed)) = (
        shelf.loaded.peek().clone(),
        shelf.probe.peek().found().cloned(),
    ) else {
        return;
    };
    match doc.view().leaving() {
        Leaving::Keep => {}
        // Held, a recording would play on unseen.
        Leaving::Release => return,
    }
    let left = shelf.left_at.peek().clone();
    let resume = match left {
        Resume::Nothing => probed.resume.clone(),
        placed @ (Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Media { .. }
        | Resume::Text { .. }
        | Resume::Book { .. }) => placed,
    };
    shelf.preloads.write().stash(Preloaded {
        probed: crate::io::Opened { resume, ..probed },
        doc,
    });
}
/// The same file is probed again: what is on screen stays until the new copy lands.
pub(super) fn reload(c: &Carry, ticket: Ticket, path: FilePath) {
    let mut shelf = c.shelf;
    shelf.wanted.set(Some(path.clone()));
    let was = shelf.probe.peek().found().cloned();
    shelf.probe.set(match was {
        Some(was) => super::super::session::Probe::Reprobing(ticket, was),
        None => super::super::session::Probe::Pending(ticket),
    });
    c.edge.submit(Job::Probe { ticket, path });
}
/// Open the files next to the open one ahead of time, on the pool's quiet lane. Nothing is opened
/// while the window has no device to upload a picture to.
pub(super) fn preload(c: &Carry, neighbours: &Neighbours) {
    let mut shelf = c.shelf;
    let around: Vec<FilePath> = [neighbours.previous.clone(), neighbours.next.clone()]
        .into_iter()
        .flatten()
        .collect();
    let wanted = if c.gpu.device().is_some() {
        around
    } else {
        Vec::new()
    };
    c.edge.preloads_wanted(&wanted);
    let fetch = shelf.preloads.write().want(&wanted);
    for path in fetch {
        let link = c.edge.link_for_preload(c.gpu.handle());
        c.edge.submit(Job::Preload { path, link });
    }
}
/// Keep `resume` as where the person is in the open file: the host knows which file that is
/// from the `Opened` request it was sent.
pub(super) fn remember(c: &Carry, resume: &Resume) {
    c.edge.request(HostRequest::Remember(resume.clone()));
}
