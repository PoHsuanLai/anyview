//! The viewer as an MPRIS player, `org.mpris.MediaPlayer2.anyview` on the session bus: the
//! control center and the media keys read it and send their controls back as typed values.

use crate::env::Env;
use crate::error::PlatformError;
use crate::media::{
    Ability, MediaControl, MediaSession, MediaState, PlaybackStatus, SeekDirection,
};
use anyview_core::{MediaTime, Percent, Volume};
use std::collections::HashMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{interface, object_server::SignalEmitter};

/// The bus name the player owns.
pub const MPRIS_NAME: &str = "org.mpris.MediaPlayer2.anyview";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const TRACK_PATH: &str = "/org/quire/Anyview1/Track";
/// MPRIS says "no track" with this object path.
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

/// [`MediaSession`] over MPRIS.
#[derive(Debug)]
pub struct MprisSession {
    connection: zbus::Connection,
    controls: UnboundedReceiver<MediaControl>,
}

impl MprisSession {
    /// Claim the player name on `env`'s session bus, showing a stopped player until the first
    /// [`MediaSession::publish`].
    pub async fn register(env: &Env) -> Result<MprisSession, PlatformError> {
        let (sender, controls) = unbounded_channel();
        let register = |error| PlatformError::bus("register the media player", error);
        let connection = env
            .session_builder()?
            .name(MPRIS_NAME)
            .map(|builder| {
                builder
                    .allow_name_replacements(false)
                    .replace_existing_names(false)
            })
            .and_then(|builder| {
                builder.serve_at(
                    OBJECT_PATH,
                    Root {
                        sender: sender.clone(),
                    },
                )
            })
            .and_then(|builder| {
                builder.serve_at(
                    OBJECT_PATH,
                    Player {
                        state: MediaState::stopped(),
                        sender,
                    },
                )
            })
            .map_err(register)?
            .build()
            .await
            .map_err(register)?;
        Ok(MprisSession {
            connection,
            controls,
        })
    }
}

impl MediaSession for MprisSession {
    async fn publish(&self, state: &MediaState) -> Result<(), PlatformError> {
        let emit = |error| PlatformError::bus("announce the player's state", error);
        let player = self
            .connection
            .object_server()
            .interface::<_, Player>(OBJECT_PATH)
            .await
            .map_err(emit)?;
        let mut held = player.get_mut().await;
        let before = std::mem::replace(&mut held.state, state.clone());
        let emitter = player.signal_emitter();
        if before.status != state.status {
            held.playback_status_changed(emitter).await.map_err(emit)?;
        }
        if metadata_of(&before) != metadata_of(state) {
            held.metadata_changed(emitter).await.map_err(emit)?;
        }
        if before.volume != state.volume {
            held.volume_changed(emitter).await.map_err(emit)?;
        }
        if before.seek != state.seek {
            held.can_seek_changed(emitter).await.map_err(emit)?;
        }
        if before.skip != state.skip {
            held.can_go_next_changed(emitter).await.map_err(emit)?;
            held.can_go_previous_changed(emitter).await.map_err(emit)?;
        }
        Ok(())
    }

    async fn next_control(&mut self) -> Option<MediaControl> {
        self.controls.recv().await
    }
}

/// The `Metadata` dictionary MPRIS clients read.
fn metadata_of(state: &MediaState) -> HashMap<String, OwnedValue> {
    let mut map = HashMap::new();
    let mut put = |key: &str, value: Value<'_>| {
        if let Ok(value) = OwnedValue::try_from(value) {
            map.insert(key.to_owned(), value);
        }
    };
    let track = format!("{TRACK_PATH}/{}", state.track.0);
    let track = ObjectPath::try_from(track.as_str())
        .unwrap_or_else(|_| ObjectPath::from_static_str_unchecked(NO_TRACK));
    put("mpris:trackid", Value::from(track));
    if let Some(length) = state.length {
        // `mpris:length` is signed microseconds.
        put("mpris:length", Value::from(micros(length.0)));
    }
    if let Some(file) = &state.file {
        put("xesam:url", Value::from(crate::file_uri(file)));
    }
    if let Some(title) = &state.title {
        put("xesam:title", Value::from(title.clone()));
    }
    if let Some(artist) = &state.artist {
        put("xesam:artist", Value::from(vec![artist.clone()]));
    }
    if let Some(album) = &state.album {
        put("xesam:album", Value::from(album.clone()));
    }
    map
}

/// A media time as MPRIS's signed microseconds, saturating.
fn micros(time: MediaTime) -> i64 {
    i64::try_from(time.0).unwrap_or(i64::MAX)
}

/// `org.mpris.MediaPlayer2`: the application itself.
struct Root {
    sender: UnboundedSender<MediaControl>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {
        let _ = self.sender.send(MediaControl::Raise);
    }

    fn quit(&self) {
        let _ = self.sender.send(MediaControl::Quit);
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> String {
        anyview_core::APP_NAME.to_owned()
    }

    #[zbus(property)]
    fn desktop_entry(&self) -> String {
        // The desktop file's id, which shells use to find the player's icon and name.
        "org.quire.Anyview".to_owned()
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["file".to_owned()]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

/// `org.mpris.MediaPlayer2.Player`: what is playing and its controls.
struct Player {
    state: MediaState,
    sender: UnboundedSender<MediaControl>,
}

impl Player {
    /// Forward a control; a closed receiver means the viewer is closing, and nobody is left to
    /// tell.
    fn control(&self, control: MediaControl) {
        let _ = self.sender.send(control);
    }

    fn skip_allowed(&self) -> bool {
        self.state.skip == Ability::Can
    }

    fn seek_allowed(&self) -> bool {
        self.state.seek == Ability::Can
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn play(&self) {
        self.control(MediaControl::Play);
    }

    fn pause(&self) {
        self.control(MediaControl::Pause);
    }

    fn play_pause(&self) {
        self.control(MediaControl::PlayPause);
    }

    fn stop(&self) {
        self.control(MediaControl::Stop);
    }

    fn next(&self) {
        if self.skip_allowed() {
            self.control(MediaControl::Next);
        }
    }

    fn previous(&self) {
        if self.skip_allowed() {
            self.control(MediaControl::Previous);
        }
    }

    /// Move by `offset` microseconds, backwards when negative.
    fn seek(&self, offset: i64) {
        if self.seek_allowed() {
            let direction = if offset < 0 {
                SeekDirection::Backward
            } else {
                SeekDirection::Forward
            };
            self.control(MediaControl::SeekBy(
                direction,
                MediaTime(offset.unsigned_abs()),
            ));
        }
    }

    /// Move to `position` microseconds, if `track` is what is playing.
    fn set_position(&self, track: ObjectPath<'_>, position: i64) {
        let current = format!("{TRACK_PATH}/{}", self.state.track.0);
        if self.seek_allowed()
            && track.as_str() == current
            && let Ok(position) = u64::try_from(position)
        {
            self.control(MediaControl::SeekTo(MediaTime(position)));
        }
    }

    /// Opening a URI is not offered: files arrive through the single-instance service.
    fn open_uri(&self, _uri: String) {}

    #[zbus(property)]
    fn playback_status(&self) -> String {
        match self.state.status {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        }
        .to_owned()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        metadata_of(&self.state)
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        micros(self.state.position)
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        f64::from(self.state.volume.percent().0) / 100.0
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) {
        let percent = (volume * 100.0).round().clamp(0.0, f64::from(u16::MAX));
        // The clamp keeps the value inside `u16`; `Volume` clamps it to its own range.
        let volume = Volume::clamped(Percent(percent as u16));
        self.control(MediaControl::SetVolume(volume));
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.skip_allowed()
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.skip_allowed()
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.seek_allowed()
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;
}
