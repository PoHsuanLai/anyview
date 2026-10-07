//! The window's way to start a player: the `MediaHost` the views are lent.

use super::actor::{MediaActor, Plan};
use super::engine::{BuiltinAbility, Chosen, Engine, builtin_ability, choose, no_sound_fact};
use super::hub::{MediaHub, SessionId};
use super::line::LiveLine;
use super::orders::Home;
use super::plugins::{MediaPlugins, PlayRoute, Playing, Reading};
use super::sink::WindowSink;
use super::snapshot::Snapshot;
use crate::runtime::{Actor, Mailbox, UiWaker};
use anyview_core::{Facts, MediaTags};
use anyview_platform::TrackSerial;
use anyview_plugin::Subject;
use anyview_ui::{MediaHost, MediaPlayback, MediaStart, MediaStarted, MediaWake, OpenError};
use std::sync::Arc;

/// Starts a player on its own media thread for each window that asks.
#[derive(Debug, Clone)]
pub struct PlayerHost {
    hub: MediaHub,
}

impl PlayerHost {
    /// A host that registers its players with `hub`.
    pub fn new(hub: MediaHub) -> PlayerHost {
        PlayerHost { hub }
    }
}

/// Wakes the window: its mailbox has news.
struct WakeWindow(MediaWake);

impl UiWaker for WakeWindow {
    fn wake(&self) {
        self.0.wake();
    }
}

impl MediaHost for PlayerHost {
    fn start(&self, start: MediaStart) -> Result<MediaStarted, OpenError> {
        let plugins = self.hub.plugins();
        // What the file says of itself. A recording the plugin cannot read may still play, so
        // facts that cannot be had are a recording with fewer rows.
        let Reading { facts, tags } = plugins.reading(&start.source, &start.sniffed);
        let subject = Subject {
            kind: start.sniffed.kind(),
            mime: Some(start.sniffed.mime()),
        };
        let audio = self.hub.inner.audio();
        let route = plugins.player(&subject);
        let ability = match &route {
            PlayRoute::Ready(_) => BuiltinAbility::CannotDecode,
            PlayRoute::Missing(_) | PlayRoute::Unserved => {
                builtin_ability(start.sniffed.kind(), start.source.path().as_path(), audio)
            }
        };
        let engine = match choose(route, ability) {
            Chosen::Mpv(host) => {
                let gpu = start.texture.gpu();
                let (Some(device), Some(queue)) = (gpu.device(), gpu.queue()) else {
                    return Err(OpenError::Media(
                        "the window has no graphics device yet".to_owned(),
                    ));
                };
                Engine::Mpv {
                    device,
                    queue,
                    audio,
                    host,
                    sink: Box::new(WindowSink(start.texture.clone())),
                }
            }
            Chosen::Builtin => Engine::Builtin { audio },
            Chosen::NoSound => {
                return Ok(unplayed(plugins, &start, no_sound_fact(), tags, facts));
            }
            Chosen::Missing(missing) => {
                return Ok(unplayed(plugins, &start, missing.fact(), tags, facts));
            }
            Chosen::Unserved => {
                return Err(OpenError::Media(
                    "no plugin plays this kind of file".to_owned(),
                ));
            }
        };
        let id: SessionId = self.hub.inner.next_id();
        let snapshot = Snapshot::new(&start.file, &tags, TrackSerial(id.0));
        let plan = Plan {
            engine,
            file: start.file.clone(),
            snapshot,
            hub: Arc::downgrade(&self.hub.inner),
            id,
            home: Home::Window,
        };
        let (mailbox, outbox) = Mailbox::new(WakeWindow(start.wake));
        let actor = Actor::spawn("anyview-media", outbox, move |wake| {
            MediaActor::start(plan, &wake)
        })
        .map_err(|error| OpenError::Media(error.to_string()))?;
        let line = Arc::new(LiveLine {
            actor,
            mailbox,
            id,
            hub: Arc::downgrade(&self.hub.inner),
        });
        self.hub.register_window(id, start.file, &line);
        Ok(MediaStarted {
            playback: MediaPlayback::Line(line),
            offer: plugins.offer(&start.sniffed, Playing::Yes),
            tags,
            facts,
            length: None,
        })
    }
}

/// A recording no player plays: its facts, with `needs` as the row that says what would.
fn unplayed(
    plugins: &MediaPlugins,
    start: &MediaStart,
    needs: anyview_core::Fact,
    tags: MediaTags,
    facts: Facts,
) -> MediaStarted {
    MediaStarted {
        playback: MediaPlayback::Missing(needs),
        offer: plugins.offer(&start.sniffed, Playing::No),
        tags,
        facts,
        length: None,
    }
}
