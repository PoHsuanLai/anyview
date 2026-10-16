//! The steps of opening a file: the probe, the peek and the open arriving, a file that failed,
//! and what a landed document tells the machines.

use super::super::carry::Carry;
use super::super::session::Probe;
use super::media::drain_media;
use super::results::{send, shown_path};
use crate::io::{HostRequest, NaturalSize, NotOpened, SizeBasis, cause_chain};
use crate::{
    Allowance, EditOffer, FileAccess, LoadIn, MarkupIn, Panel, PictureEditIn, PictureEditing,
    Presentation, Stage, StageIn, Ticket, ViewerIn,
};
use anyview_core::{PixelLen, PixelSize, Resume};
use dioxus::prelude::*;

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
pub(super) fn couldnt(c: &Carry, what: &str, error: &crate::OpenError) {
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

pub(super) fn probed(c: &Carry, ticket: Ticket, result: Result<crate::Opened, crate::OpenError>) {
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

pub(super) fn peeked(
    c: &Carry,
    ticket: Ticket,
    result: Result<Option<crate::LoadedDoc>, crate::OpenError>,
) {
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

pub(super) fn opened(
    c: &Carry,
    ticket: Ticket,
    result: Result<crate::LoadedDoc, crate::OpenError>,
) {
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
pub(in crate::views) fn landed(c: &Carry, ticket: Ticket, doc: &crate::LoadedDoc) {
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
        width: PixelLen(page.width.0.saturating_add(super::super::panel::WIDTH)),
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
