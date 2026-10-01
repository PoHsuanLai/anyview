//! The player's side of the media stage: what it reports and what it is told. The events mirror
//! mpv-wgpu-player's, as this crate's own types so the stage depends on no player.

use anyview_core::{MediaLength, MediaTime, Percent, TrackChoice, Volume};

/// Why the file stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndReason {
    /// Reached the end.
    Eof,
    /// Stopped by command.
    Stop,
    /// The player is quitting.
    Quit,
    /// The playlist entry was redirected.
    Redirect,
    /// The player reported an error.
    Error,
}

/// Whether the file is advancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pace {
    /// Frames and audio advance.
    Playing,
    /// Picture and audio are held.
    Paused,
}

/// A notification from the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEvent {
    /// The file finished opening; it is this long.
    Loaded { length: MediaLength },
    /// The file stopped.
    Ended(EndReason),
    /// Pause state changed, whoever changed it.
    Playback(Pace),
    /// A seek landed and playback restarted from the new position.
    SeekDone,
    /// The demuxer cache level moved.
    Buffering(Percent),
    /// The track list or the selected tracks changed.
    TracksChanged,
    /// The volume changed, whoever asked for it.
    VolumeChanged(Volume),
}

/// Which tracks a choice is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackKind {
    /// The audio track.
    Audio,
    /// The subtitle track.
    Subtitles,
}

/// Which way a frame step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameDirection {
    /// To the next frame.
    Forward,
    /// To the previous frame.
    Backward,
}

/// A command for the player, as data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerCommand {
    /// Play or hold.
    SetPlayback(Pace),
    /// Seek to this position from the start.
    Seek(MediaTime),
    /// Set the volume.
    SetVolume(Volume),
    /// Choose a track.
    SelectTrack {
        kind: TrackKind,
        choice: TrackChoice,
    },
    /// Step one frame; only meaningful while held.
    FrameStep(FrameDirection),
}
