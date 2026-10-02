//! What a worker made, as input to the machines. Every result carries the ticket of the file it
//! was asked for, and one for a file the person has left is dropped here or by the machine.

use super::carry::Carry;
use super::session::Probe;
use crate::families::{FoundHits, Held};
use crate::io::{Done, Job};
use crate::{
    Freshness, LoadIn, NavigateIn, Stage, StageIn, TextIn, TextStage, Ticket, TypedText, ViewerIn,
    freshness,
};
use anyview_core::{FilePath, Resume};
use dioxus::prelude::*;
use std::sync::Arc;

/// Feed the result `done` to the window's machines.
pub(super) fn arrived(done: Done, c: &Carry) {
    match done {
        Done::Probed { ticket, result } => probed(c, ticket, result),
        Done::Peeked { ticket, result } => peeked(c, ticket, result),
        Done::Opened { ticket, result } => opened(c, ticket, result),
        Done::Lines { ticket, result } => {
            let mut lines = c.shelf.lines;
            if let (Some((held, _)), Ok(window)) = (c.shelf.shown_now(), result)
                && held == ticket
            {
                lines.set(Some(Held(Arc::new(window))));
            }
        }
        Done::Found {
            ticket,
            query,
            result,
        } => found(c, ticket, query, result.unwrap_or_default()),
        Done::Preloaded { path, loaded } => {
            let mut preloads = c.shelf.preloads;
            preloads.write().arrived(&path, loaded);
        }
        Done::Stamped { path, stamp } => stamped(c, &path, stamp),
        Done::Folder { path, result } => folder(c, &path, result),
        Done::Changed { path } => {
            if shown_path(c).as_ref() == Some(&path) {
                c.edge.submit(Job::Stat { path });
            }
        }
    }
}

/// The file on screen.
fn shown_path(c: &Carry) -> Option<FilePath> {
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

fn failed(c: &Carry, ticket: Ticket, error: &crate::OpenError) {
    send(
        c,
        ViewerIn::Load(LoadIn::Failed {
            ticket,
            reason: error.failure(),
        }),
    );
}

fn probed(c: &Carry, ticket: Ticket, result: Result<crate::Probed, crate::OpenError>) {
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
                Probe::Reprobing(..) => crate::Probed {
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
        Ok(None) | Err(_) => send(c, ViewerIn::Load(LoadIn::PeekFailed { ticket })),
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
    for input in told_of(c, doc) {
        send(c, ViewerIn::Stage(input));
    }
}

/// The inputs the stage is given now that `doc` is on screen.
fn told_of(c: &Carry, doc: &crate::LoadedDoc) -> Vec<StageIn> {
    let Some(dispatch) = c.dispatch() else {
        return Vec::new();
    };
    let stage = dispatch.machine.state().peek().stage.clone();
    doc.view().arrived(&stage)
}

/// A search answered: the hits are kept for the view, and the stage is told how many there are
/// and which is nearest the reader.
fn found(c: &Carry, ticket: Ticket, query: TypedText, hits: FoundHits) {
    let mut shelf = c.shelf;
    if shelf.loaded.peek().as_ref().map(|(held, _)| *held) != Some(ticket) {
        return;
    }
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let Stage::Text(TextStage::Finding {
        query: asked,
        place,
        ..
    }) = dispatch.machine.state().peek().stage.clone()
    else {
        return;
    };
    if asked != query {
        return;
    }
    let (count, nearest) = (hits.count(), hits.nearest(place.line));
    shelf.hits.set(Some(Held(Arc::new(hits))));
    dispatch.send(ViewerIn::Stage(StageIn::Text(TextIn::Results {
        query,
        count,
        nearest,
    })));
}

/// The stamp of the file on screen as the disk has it now: a file that differs is opened again.
fn stamped(c: &Carry, path: &FilePath, stamp: Option<anyview_core::FileStamp>) {
    let Some(probed) = c.shelf.probe.peek().found().cloned() else {
        return;
    };
    if probed.source.path() != path {
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
