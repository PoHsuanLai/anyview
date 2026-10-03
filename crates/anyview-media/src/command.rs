//! What the viewer tells the player, as data.

use anyview_core::{ChapterIndex, FilePath, MediaTime, Speed, StreamKind, TrackChoice, Volume};
use ds_core::word::Word;
use std::num::NonZeroU32;

/// Whether the recording is advancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Pace {
    /// Frames and audio advance.
    Playing,
    /// Picture and audio are held.
    Paused,
}

/// Which way a step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Direction {
    /// Towards the end.
    Forward,
    /// Towards the start.
    Backward,
}

/// The rectangle the player scales and letterboxes the picture into, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureSlot {
    /// No picture is wanted: the player skips the GPU work (a background session).
    Empty,
    /// A picture of this size.
    Sized {
        /// Width in physical pixels.
        width: NonZeroU32,
        /// Height in physical pixels.
        height: NonZeroU32,
    },
}

/// What a saved frame holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ShotContent {
    /// The decoded picture at its own resolution.
    Video,
    /// The same with the shown subtitles drawn in.
    Subtitles,
}

/// One instruction for the player.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaCommand {
    /// Where the picture goes.
    Slot(PictureSlot),
    /// Play or hold.
    SetPlayback(Pace),
    /// Move to a position, counted from the start.
    Seek(MediaTime),
    /// Set the output level.
    SetVolume(Volume),
    /// Set the speed.
    SetSpeed(Speed),
    /// Play at the next preset speed above or below the one playing.
    StepSpeed(Direction),
    /// Play a track of a kind, or none.
    SelectTrack {
        /// Which family of stream.
        kind: StreamKind,
        /// Which track.
        choice: TrackChoice,
    },
    /// Play the next track of a kind, wrapping (subtitles pass through "none").
    CycleTrack(StreamKind),
    /// Jump to the next or previous chapter.
    StepChapter(Direction),
    /// Jump to a chapter.
    GoToChapter(ChapterIndex),
    /// Show the next or previous frame; the player holds.
    FrameStep(Direction),
    /// Save the frame on screen to `to`; the extension picks the format.
    Screenshot {
        /// The file to write.
        to: FilePath,
        /// What the frame holds.
        content: ShotContent,
    },
    /// End the session.
    Close,
}
