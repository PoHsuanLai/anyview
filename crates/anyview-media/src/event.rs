//! What the player reports, as data.

use crate::command::Pace;
use anyview_core::{
    FilePath, MediaChapter, MediaLength, MediaTime, MediaTrack, Percent, Speed, VideoPresence,
    Volume,
};
use ds_core::word::Word;

/// Why a file stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum EndReason {
    /// Reached the end.
    Eof,
    /// Stopped by command.
    Stop,
    /// The player is quitting.
    Quit,
    /// The entry was redirected.
    Redirect,
    /// The player reported an error.
    Error,
}

/// One notification from the player.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaEvent {
    /// The file finished opening. A length is missing for a stream with none.
    Loaded {
        /// How long the recording runs.
        length: Option<MediaLength>,
    },
    /// The file stopped.
    Ended(EndReason),
    /// Pause state changed, whoever changed it.
    Playback(Pace),
    /// A seek landed and playback restarted from the new position.
    SeekDone,
    /// The demuxer cache level moved.
    Buffering(Percent),
    /// Where playback is, at most ten times a second while it moves.
    Position(MediaTime),
    /// The track list or the selected tracks changed.
    Tracks(Vec<MediaTrack>),
    /// The chapter list changed.
    Chapters(Vec<MediaChapter>),
    /// The volume changed, whoever asked for it.
    Volume(Volume),
    /// The speed changed.
    Speed(Speed),
    /// Whether the recording shows a picture, now.
    Picture(VideoPresence),
    /// A frame was saved.
    ShotSaved(FilePath),
    /// A frame could not be saved.
    ShotFailed {
        /// The file that was asked for.
        to: FilePath,
        /// What the player said.
        reason: String,
    },
    /// The player could not play the recording at all.
    Failed(String),
    /// The player refused one instruction; playback goes on.
    Refused(String),
}
