//! The window's way to start a player: the `MediaHost` the views are lent.

use super::actor::{MediaActor, Plan};
use super::hub::{MediaHub, SessionId};
use super::line::LiveLine;
use super::orders::Home;
use super::sink::WindowSink;
use super::snapshot::Snapshot;
use crate::runtime::{Actor, Mailbox, UiWaker};
use anyview_core::{FactLabel, FactValue, Facts, MediaTags};
use anyview_media::MediaProbe;
use anyview_platform::TrackSerial;
use anyview_ui::{MediaHost, MediaStart, MediaStarted, MediaWake, OpenError};
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

/// The rows of facts a probe gives: how long it runs, how it is made and what it says of itself.
pub(super) fn facts_of(probe: &MediaProbe) -> Facts {
    let mut facts = Facts::empty();
    if let Some(length) = probe.length {
        facts = facts.with(FactLabel::Duration, FactValue::duration(length));
    }
    if let Some(video) = &probe.video {
        facts = facts
            .with(FactLabel::Dimensions, FactValue::dimensions(video.size))
            .with(FactLabel::Codec, FactValue::text(video.codec.clone()));
    }
    if let Some(audio) = &probe.audio {
        let label = match probe.video {
            Some(_) => FactLabel::AudioCodec,
            None => FactLabel::Codec,
        };
        facts = facts.with(label, FactValue::text(audio.codec.clone()));
        if let Some(rate) = audio.bitrate {
            facts = facts.with(FactLabel::Bitrate, FactValue::bitrate(rate));
        }
    }
    for (label, tag) in [
        (FactLabel::Title, &probe.tags.title),
        (FactLabel::Author, &probe.tags.artist),
        (FactLabel::Album, &probe.tags.album),
    ] {
        if let Some(text) = tag {
            facts = facts.with(label, FactValue::text(text.clone()));
        }
    }
    facts
}

impl MediaHost for PlayerHost {
    fn start(&self, start: MediaStart) -> Result<MediaStarted, OpenError> {
        let gpu = start.texture.gpu();
        let (Some(device), Some(queue)) = (gpu.device(), gpu.queue()) else {
            return Err(OpenError::Media(
                "the window has no graphics device yet".to_owned(),
            ));
        };
        // What the file says of itself. A recording libav cannot read may still play, so a probe
        // that fails is a recording with no facts.
        let probe = anyview_media::probe(start.file.as_path()).ok();
        let tags: MediaTags = probe
            .as_ref()
            .map(|probe| probe.tags.clone())
            .unwrap_or_default();
        let facts = probe.as_ref().map(facts_of).unwrap_or_default();
        let length = probe.as_ref().and_then(|probe| probe.length);

        let id: SessionId = self.hub.inner.next_id();
        let snapshot = Snapshot::new(&start.file, &tags, TrackSerial(id.0));
        let plan = Plan {
            device,
            queue,
            audio: self.hub.inner.audio(),
            file: start.file.clone(),
            sink: Box::new(WindowSink(start.texture.clone())),
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
            line,
            tags,
            facts,
            length,
        })
    }
}
