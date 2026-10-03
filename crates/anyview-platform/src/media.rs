//! The now-playing entry the rest of the desktop shows (the control center, a media key) and the
//! controls that come back from it.

use crate::error::PlatformError;
use anyview_core::{FilePath, MediaLength, MediaTime, Volume};
use ds_core::word::Word;
use std::future::Future;

/// Whether the recording is moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PlaybackStatus {
    /// Playing.
    Playing,
    /// Paused, position kept.
    Paused,
    /// Nothing loaded or playback ended.
    Stopped,
}

/// Whether the desktop may ask for something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Ability {
    /// It may.
    Can,
    /// It may not; the desktop greys the control.
    Cannot,
}

/// A number that changes when a different recording starts, so a client tells a new track from
/// a position jump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TrackSerial(pub u32);

/// What the desktop is told about the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaState {
    /// Playing, paused or stopped.
    pub status: PlaybackStatus,
    /// Which recording this is.
    pub track: TrackSerial,
    /// The file playing, when it is one.
    pub file: Option<FilePath>,
    /// The title tag, or the file name when the file has none.
    pub title: Option<String>,
    /// The artist tag.
    pub artist: Option<String>,
    /// The album tag.
    pub album: Option<String>,
    /// How long the recording runs, once known.
    pub length: Option<MediaLength>,
    /// Where playback is.
    pub position: MediaTime,
    /// The output level.
    pub volume: Volume,
    /// Whether the desktop may seek.
    pub seek: Ability,
    /// Whether the desktop may skip to the next or previous file.
    pub skip: Ability,
}

impl MediaState {
    /// Nothing playing: stopped, no recording, no controls.
    pub fn stopped() -> MediaState {
        MediaState {
            status: PlaybackStatus::Stopped,
            track: TrackSerial::default(),
            file: None,
            title: None,
            artist: None,
            album: None,
            length: None,
            position: MediaTime::default(),
            volume: Volume::clamped(anyview_core::Percent(100)),
            seek: Ability::Cannot,
            skip: Ability::Cannot,
        }
    }
}

/// Which way a relative seek goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum SeekDirection {
    /// Towards the end.
    Forward,
    /// Towards the start.
    Backward,
}

/// What the desktop asks of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaControl {
    /// Start or resume.
    Play,
    /// Pause.
    Pause,
    /// Pause if playing, else play.
    PlayPause,
    /// Stop and rewind.
    Stop,
    /// The next file in the sequence.
    Next,
    /// The previous file in the sequence.
    Previous,
    /// Move this far from where playback is.
    SeekBy(SeekDirection, MediaTime),
    /// Move to this position.
    SeekTo(MediaTime),
    /// Set the output level.
    SetVolume(Volume),
    /// Bring the window to the front.
    Raise,
    /// Close the viewer.
    Quit,
}

/// Publish the player's state and receive the desktop's controls.
pub trait MediaSession {
    /// Make `state` what the desktop shows. Cheap to call on every change.
    fn publish(&self, state: &MediaState)
    -> impl Future<Output = Result<(), PlatformError>> + Send;

    /// The next control, or `None` when none can arrive any more.
    fn next_control(&mut self) -> impl Future<Output = Option<MediaControl>> + Send;
}
