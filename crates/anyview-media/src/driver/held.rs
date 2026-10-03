//! The session in whichever state it is in, so a driver holds one value across the moves.

use crate::command::PictureSlot;
use crate::error::MediaError;
use crate::session::{Idle, Loaded, Opened, Opening, Report, Session};

/// A session in one of its states.
#[derive(Debug)]
pub(super) enum Held {
    Idle(Session<Idle>),
    Opening(Session<Opening>),
    Loaded(Session<Loaded>),
}

impl Held {
    pub(super) fn set_slot(&mut self, slot: PictureSlot) -> Result<(), MediaError> {
        match self {
            Held::Idle(session) => session.set_slot(slot),
            Held::Opening(session) => session.set_slot(slot),
            Held::Loaded(session) => session.set_slot(slot),
        }
    }

    pub(super) fn picture(&self) -> Option<&wgpu::TextureView> {
        match self {
            Held::Idle(session) => session.picture(),
            Held::Opening(session) => session.picture(),
            Held::Loaded(session) => session.picture(),
        }
    }

    /// Poll the session; an opening one may become loaded or fall back to idle.
    pub(super) fn poll(self) -> (Held, Report, Moved) {
        match self {
            Held::Idle(session) => (Held::Idle(session), Report::quiet(), Moved::Stayed),
            Held::Opening(session) => match session.poll() {
                Opened::Waiting(session, report) => (Held::Opening(session), report, Moved::Stayed),
                Opened::Loaded(session, report) => (Held::Loaded(session), report, Moved::ToLoaded),
                Opened::Failed(session, report) => (Held::Idle(session), report, Moved::Stayed),
            },
            Held::Loaded(mut session) => {
                let report = session.poll();
                (Held::Loaded(session), report, Moved::Stayed)
            }
        }
    }
}

/// Whether a poll moved the session into the loaded state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Moved {
    Stayed,
    ToLoaded,
}
