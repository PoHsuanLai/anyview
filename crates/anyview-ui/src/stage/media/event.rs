//! The player's side of the media stage: what it reports and what it is told. The events mirror
//! mpv-wgpu-player's, as this crate's own types so the stage depends on no player.

use anyview_core::{ChapterIndex, MediaLength, MediaTime, Percent, Speed, TrackChoice, Volume};
use ds_core::word::Word;

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
    /// The recording's length became known after `Loaded` said it had none.
    LengthKnown(MediaLength),
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

/// Which way a step goes: a frame, a chapter or a speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepDirection {
    /// To the next frame.
    Forward,
    /// To the previous frame.
    Backward,
}

/// Which end of a trim a mark sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TrimEdge {
    /// Where the kept part begins.
    Start,
    /// Where the kept part ends.
    End,
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
    FrameStep(StepDirection),
    /// Play at this speed.
    SetSpeed(Speed),
    /// Play the next preset speed up or down from the one playing.
    StepSpeed(StepDirection),
    /// Play the next track of a kind, wrapping.
    CycleTrack(TrackKind),
    /// Jump to the next or previous chapter.
    StepChapter(StepDirection),
    /// Jump to a chapter.
    GoToChapter(ChapterIndex),
}
