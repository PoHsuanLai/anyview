//! The views' way to start a player: the `MediaHost` a window or a pane is lent.

use super::actor::{MediaActor, Plan};
use super::engine::{BuiltinAbility, Chosen, Engine, builtin_ability, choose, no_sound_fact};
use super::hub::{MediaHub, SessionId};
use super::line::LiveLine;
use super::orders::Home;
use super::plugins::{PlayRoute, PlayerPlugins, Playing, Reading};
use super::sink::WindowSink;
use super::snapshot::Snapshot;
use anyview_core::{Facts, FormatKind, MediaTags};
use anyview_platform::{Ability, Artwork, TrackSerial};
use anyview_plugin::Subject;
use anyview_runtime::{Actor, Mailbox, UiWaker};
use anyview_ui::{MediaHost, MediaPlayback, MediaStart, MediaStarted, MediaWake, Need, OpenError};
use ds::components::content::image_source::ImageSource;
use std::sync::Arc;

/// Starts a player on its own media thread for each window or pane that asks.
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
        let Reading { facts, tags, cover } = plugins.reading(&start.source, &start.sniffed);
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
                return Ok(unplayed(
                    plugins,
                    &start,
                    Need::passive(no_sound_fact()),
                    tags,
                    facts,
                    cover,
                ));
            }
            Chosen::Missing(missing) => {
                return Ok(unplayed(
                    plugins,
                    &start,
                    plugins.need_of(&missing),
                    tags,
                    facts,
                    cover,
                ));
            }
            Chosen::Unserved => {
                return Err(OpenError::Media(
                    "no plugin plays this kind of file".to_owned(),
                ));
            }
        };
        let id: SessionId = self.hub.inner.next_id();
        let art = cover.as_ref().map(|png| Artwork {
            png: Arc::from(png.as_slice()),
        });
        let snapshot = Snapshot::new(&start.file, &tags, art, Ability::Can, TrackSerial(id.0));
        let plan = Plan {
            engine,
            file: start.file.clone(),
            snapshot,
            hub: Arc::downgrade(&self.hub.inner),
            id,
            home: Home::Window,
        };
        let (mailbox, outbox) = Mailbox::new(WakeWindow(start.wake));
        let posting = outbox.clone();
        let actor = Actor::spawn("anyview-media", outbox, move |wake| {
            MediaActor::start(plan, &wake)
        })
        .map_err(|error| OpenError::Media(error.to_string()))?;
        let line = Arc::new(LiveLine {
            actor,
            mailbox,
            outbox: posting,
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
            cover: cover.map(|png| ImageSource::png(&png)),
        })
    }
}

/// A recording no player plays: its facts, with `needs` as the row that says what would. Its
/// picture is the cover an audio file carries (`cover`, read with the facts) or the frame of a
/// video a decoder makes.
fn unplayed(
    plugins: &dyn PlayerPlugins,
    start: &MediaStart,
    needs: Need,
    tags: MediaTags,
    facts: Facts,
    cover: Option<Vec<u8>>,
) -> MediaStarted {
    let cover = card_picture(start.sniffed.kind(), cover, || poster(plugins, start));
    MediaStarted {
        playback: MediaPlayback::Missing(needs),
        offer: plugins.offer(&start.sniffed, Playing::No),
        tags,
        facts,
        length: None,
        cover,
    }
}

/// The picture of the card of a recording nothing plays: an audio file's own `cover`, and for
/// anything else the `frame` a decoder makes (asked only then).
fn card_picture(
    kind: FormatKind,
    cover: Option<Vec<u8>>,
    frame: impl FnOnce() -> Option<ImageSource>,
) -> Option<ImageSource> {
    if kind == FormatKind::Audio {
        cover.map(|png| ImageSource::png(&png))
    } else {
        frame()
    }
}

/// The frame of a video nothing plays, for its card: a video's only picture is the one a program
/// that can decode it makes, and a file with none shows the plate.
fn poster(plugins: &dyn PlayerPlugins, start: &MediaStart) -> Option<ImageSource> {
    (start.sniffed.kind() == FormatKind::Video)
        .then(|| plugins.poster(&start.source, &start.sniffed))
        .flatten()
        .map(|png| ImageSource::png(&png))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_shows_an_audio_files_cover_and_a_videos_frame() {
        let png = |bytes: &[u8]| ImageSource::png(bytes).0;
        let cover = Some(b"cover".to_vec());
        let none = || -> Option<ImageSource> { panic!("no frame is made for an audio file") };
        let frame = || Some(ImageSource::png(b"frame"));
        let audio = |cover| card_picture(FormatKind::Audio, cover, none).map(|source| source.0);
        assert_eq!(audio(cover.clone()), Some(png(b"cover")));
        assert_eq!(audio(None), None);
        let video = card_picture(FormatKind::Video, cover, frame).map(|source| source.0);
        assert_eq!(video, Some(png(b"frame")));
    }
}
