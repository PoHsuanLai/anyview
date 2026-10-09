//! The effects that wait on something: a probe's result is announced to the load machine only
//! after a draw that knew its kind, and a file is opened only once the window has a device to
//! upload a picture to. Each reads signals the outputs and results write, so it runs when what it
//! waits for has come.

use super::arrive::landed;
use super::carry::Carry;
use super::session::{Probe, family};
use super::shelf::FirstLoad;
use crate::families::flow_of;
use crate::io::{HostRequest, Job};
use crate::{LoadIn, StageFamily, ViewerIn};
use dioxus::prelude::*;

/// A probe that arrived is announced to the load machine, which installs the stage and starts
/// the first frame and the open; then the stage is given where the person was last time.
pub(super) fn use_announce(c: &Carry) {
    let c = c.clone();
    use_effect(move || {
        let mut probe = c.shelf.probe;
        let Probe::Arrived(ticket, probed) = probe() else {
            return;
        };
        let mut first = c.shelf.first;
        if *first.peek() == FirstLoad::Unseen {
            first.set(FirstLoad::Is(ticket));
        }
        let stage = family(&probed);
        let flow = flow_of(probed.sniffed.kind());
        let resume = probed.resume.clone();
        c.edge.request(HostRequest::Opened(probed.clone()));
        c.edge
            .request(HostRequest::Watch(probed.source.path().clone()));
        probe.set(Probe::Announced(ticket, probed));
        let Some(dispatch) = c.dispatch() else {
            return;
        };
        dispatch.send(ViewerIn::Load(LoadIn::Probed {
            ticket,
            flow,
            stage,
        }));
        let showing = dispatch.machine.state().peek().stage.clone();
        if let Some(input) = showing.restoring(&resume) {
            dispatch.send(ViewerIn::Stage(input));
        }
    });
}

/// The first frame and the open wait for the window's device when the file is a picture, then
/// each is handed to a worker with a texture of its own. A file already open (opened ahead, or
/// the copy on screen while a changed file loads again) needs neither.
pub(super) fn use_work(c: &Carry) {
    let c = c.clone();
    let ready = c.gpu.device().is_some();
    use_effect(use_reactive!(|ready| {
        let mut shelf = c.shelf;
        // Read, not peeked: the effect runs again when a ticket is asked for.
        let (peeking, opening, probe) = (
            shelf.peeking.cloned(),
            shelf.opening.cloned(),
            shelf.probe.cloned(),
        );
        let Probe::Announced(held, probed) = probe else {
            return;
        };
        // A picture is uploaded to the window's device, and a player draws on it: neither can
        // start before the window has one.
        let needs_device = match probed.family {
            StageFamily::Raster | StageFamily::Media => true,
            StageFamily::Pdf
            | StageFamily::Text
            | StageFamily::Table
            | StageFamily::Tree
            | StageFamily::PeekOnly => false,
        };
        if needs_device && !ready {
            return;
        }
        if let Some(ticket) = peeking.filter(|ticket| *ticket == held) {
            shelf.peeking.set(None);
            if shelf.shown_now().is_none() {
                let link = c.edge.link(c.gpu.handle());
                c.edge.submit(Job::Peek {
                    ticket,
                    probed: probed.clone(),
                    link,
                });
            }
        }
        if let Some(ticket) = opening.filter(|ticket| *ticket == held) {
            shelf.opening.set(None);
            let ahead = shelf
                .loaded
                .peek()
                .clone()
                .filter(|(landed, _)| *landed == ticket);
            match ahead {
                Some((_, doc)) => landed(&c, ticket, &doc),
                None => {
                    let link = c.edge.link(c.gpu.handle());
                    c.edge.submit(Job::Open {
                        ticket,
                        probed,
                        link,
                    });
                }
            }
        }
    }));
}
