//! The media thread's actor: one player, the instructions it is sent and the news it makes. The
//! player's logic is a `anyview_media::MediaDriver`; this is the part only the program can write: where
//! the news goes (the window's mailbox, the desktop's now-playing entry, a waiting export) and
//! what ends a session.

use super::engine::Engine;
use super::hub::{Inner, SessionId};
use super::map::{Opened, notices_of};
use super::orders::Home;
use super::snapshot::{Change, Snapshot};
use crate::runtime::{ActorBody, ActorWake, Flow, Outbox};
use anyview_core::FilePath;
use anyview_media::{Continuation, EndReason, MediaCommand, MediaDriver, MediaEvent};
use anyview_ui::MediaNotice;
use std::sync::Weak;

/// Everything the actor is built from, on the thread that builds it.
pub(super) struct Plan {
    pub(super) engine: Engine,
    pub(super) file: FilePath,
    pub(super) snapshot: Snapshot,
    pub(super) hub: Weak<Inner>,
    pub(super) id: SessionId,
    pub(super) home: Home,
}

/// One player on the media thread.
pub(super) struct MediaActor {
    /// The player, or why it could not be made: the failure is reported on the first wake.
    driver: Result<Box<dyn MediaDriver>, Option<String>>,
    snapshot: Snapshot,
    hub: Weak<Inner>,
    id: SessionId,
    home: Home,
    opened: Opened,
}

impl MediaActor {
    /// Build the player. Runs on the actor's thread: the player lives and dies there.
    pub(super) fn start(plan: Plan, wake: &ActorWake<MediaCommand>) -> MediaActor {
        let Plan {
            engine,
            file,
            snapshot,
            hub,
            id,
            home,
        } = plan;
        let for_player = wake.clone();
        let driver = engine
            .start(&file, move || for_player.wake())
            .map_err(|error| Some(error.to_string()));
        if driver.is_err() {
            // Nothing will wake it: ask for the turn that reports the failure.
            wake.wake();
        }
        MediaActor {
            driver,
            snapshot,
            hub,
            id,
            home,
            opened: Opened::Not,
        }
    }

    /// Hand each event to whoever wants it.
    fn deliver(&mut self, events: Vec<MediaEvent>, out: &Outbox<MediaNotice>) {
        let hub = self.hub.upgrade();
        let mut publish = false;
        for event in &events {
            if matches!(event, MediaEvent::Loaded { .. }) {
                self.opened = Opened::Yes;
            }
            if self.snapshot.apply(event) == Change::Now {
                publish = true;
            }
            self.route(event, hub.as_deref());
            match self.home {
                Home::Window => notices_of(event, self.opened)
                    .into_iter()
                    .for_each(|notice| out.send(notice)),
                // Nobody is looking: there is no window to tell.
                Home::Background => {}
            }
        }
        if let (true, Some(hub)) = (publish, hub.as_deref()) {
            hub.state(self.id, self.snapshot.state().clone());
        }
    }

    /// What an event means to the program beyond the window.
    fn route(&self, event: &MediaEvent, hub: Option<&Inner>) {
        match (event, hub) {
            (MediaEvent::ShotSaved(to), Some(hub)) => hub.shot_done(self.id, to, Ok(())),
            (MediaEvent::ShotFailed { to, reason }, Some(hub)) => {
                hub.shot_done(self.id, to, Err(reason.clone()));
            }
            (MediaEvent::Refused(reason), _) => eprintln!("anyview: the player refused: {reason}"),
            (MediaEvent::Ended(EndReason::Eof) | MediaEvent::Failed(_), Some(hub)) => {
                match self.home {
                    // A session with no window ends when its recording does: nothing else would.
                    Home::Background => hub.finished(self.id),
                    Home::Window => {}
                }
            }
            (
                MediaEvent::Loaded { .. }
                | MediaEvent::Length(_)
                | MediaEvent::Ended(_)
                | MediaEvent::Playback(_)
                | MediaEvent::SeekDone
                | MediaEvent::Buffering(_)
                | MediaEvent::Position(_)
                | MediaEvent::Tracks(_)
                | MediaEvent::Chapters(_)
                | MediaEvent::Volume(_)
                | MediaEvent::Speed(_)
                | MediaEvent::Picture(_)
                | MediaEvent::ShotSaved(_)
                | MediaEvent::ShotFailed { .. }
                | MediaEvent::Failed(_),
                _,
            ) => {}
        }
    }
}

impl ActorBody for MediaActor {
    type Command = MediaCommand;
    type Event = MediaNotice;

    fn command(&mut self, command: MediaCommand, out: &Outbox<MediaNotice>) -> Flow {
        let Ok(driver) = &mut self.driver else {
            return Flow::Continue;
        };
        let handled = driver.command(command);
        self.deliver(handled.events, out);
        match handled.then {
            Continuation::Keep => Flow::Continue,
            Continuation::Close => Flow::Quit,
        }
    }

    fn woken(&mut self, out: &Outbox<MediaNotice>) {
        let events = match &mut self.driver {
            Ok(driver) => driver.woken(),
            Err(failure) => failure
                .take()
                .map(|reason| vec![MediaEvent::Failed(reason)])
                .unwrap_or_default(),
        };
        self.deliver(events, out);
    }
}
