//! The media stage's states, inputs and outputs.

use super::event::{PlayerCommand, PlayerEvent, StepDirection, TrackKind, TrimEdge};
use anyview_core::{ChapterIndex, MediaLength, MediaTime, Percent, Speed, TrackChoice, Volume};
use ds_core::word::Word;

/// What to do when a scrub ends: scrubbing pauses the player so the picture follows the pointer,
/// and the person's choice from before is restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AfterScrub {
    /// It was playing: play on from where the scrub ended.
    Play,
    /// It was held (or had ended): stay held there.
    Stay,
}

/// Why media failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum MediaError {
    /// The file would not open.
    OpenFailed,
    /// The player gave up part-way through.
    PlaybackFailed,
}

/// Where playback is. `at` is the position and `length` how long the recording runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaStage {
    /// Waiting for the player to open the file.
    #[default]
    Opening,
    /// Advancing.
    Playing { at: MediaTime, length: MediaLength },
    /// Held.
    Paused { at: MediaTime, length: MediaLength },
    /// The slider is being dragged: the player is held at `to` while `from` is where the drag
    /// began.
    Scrubbing {
        from: MediaTime,
        to: MediaTime,
        length: MediaLength,
        resume: AfterScrub,
    },
    /// Played to the end.
    Ended { at: MediaTime, length: MediaLength },
    /// Could not play.
    Failed(MediaError),
}

impl MediaStage {
    /// How long the recording runs, once the player has said.
    pub fn length(&self) -> Option<MediaLength> {
        match self {
            MediaStage::Playing { length, .. }
            | MediaStage::Paused { length, .. }
            | MediaStage::Scrubbing { length, .. }
            | MediaStage::Ended { length, .. } => Some(*length),
            MediaStage::Opening | MediaStage::Failed(_) => None,
        }
    }

    /// The same state with the length the player has now said; a stage that is not showing a
    /// recording yet has no length to change.
    pub(super) fn with_length(self, length: MediaLength) -> MediaStage {
        match self {
            MediaStage::Playing { at, .. } => MediaStage::Playing { at, length },
            MediaStage::Paused { at, .. } => MediaStage::Paused { at, length },
            MediaStage::Scrubbing {
                from, to, resume, ..
            } => MediaStage::Scrubbing {
                from,
                to,
                length,
                resume,
            },
            MediaStage::Ended { at, .. } => MediaStage::Ended { at, length },
            MediaStage::Opening | MediaStage::Failed(_) => self,
        }
    }
}

/// What moves the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaIn {
    /// The player reported something.
    Player(PlayerEvent),
    /// The player's position, polled while it plays.
    Position(MediaTime),
    /// Play or pause; from the end it plays again from the start.
    Toggle,
    /// Back by one seek step.
    SeekBack,
    /// Forward by one seek step.
    SeekForward,
    /// To a position.
    SeekTo(MediaTime),
    /// The slider was grabbed.
    ScrubStart,
    /// The slider moved to this position.
    ScrubTo(MediaTime),
    /// The slider was let go.
    ScrubEnd,
    /// The scrub was abandoned: back to where it began.
    ScrubCancel,
    /// Set the volume.
    SetVolume(Volume),
    /// Choose a track.
    Select {
        kind: TrackKind,
        choice: TrackChoice,
    },
    /// Step one frame.
    FrameStep(StepDirection),
    /// Play at this speed.
    SetSpeed(Speed),
    /// Play the next preset speed up or down.
    StepSpeed(StepDirection),
    /// Play the next track of a kind, wrapping.
    CycleTrack(TrackKind),
    /// Jump to the next or previous chapter.
    StepChapter(StepDirection),
    /// Jump to a chapter.
    GoToChapter(ChapterIndex),
    /// Mark where a trim begins or ends, at the position now.
    Mark(TrimEdge),
    /// Put the settings the person left back: the position, the volume and the tracks. They
    /// are told to the player at once and it applies them when the file opens.
    Restore {
        at: MediaTime,
        volume: Volume,
        audio: TrackChoice,
        subtitles: TrackChoice,
    },
    /// The player could not be started at all.
    Failed(MediaError),
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for MediaIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        MediaIn::Elapsed
    }
}

/// What the stage wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaOut {
    /// A command for the player.
    Command(PlayerCommand),
    /// Show how full the cache is.
    Buffering(Percent),
    /// Show the new volume on the capsule and keep it for next time.
    VolumeChanged(Volume),
    /// Read the track list again for the panel.
    TracksChanged,
    /// The position a trim's end is marked at: the window keeps the marks the export uses.
    Marked { edge: TrimEdge, at: MediaTime },
}

/// Whether the player does a thing, so the window offers its control or leaves it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ControlOffer {
    /// The player does it: its control is offered.
    #[default]
    Offered,
    /// The player cannot: no control, command or key for it is offered.
    Withheld,
}

/// What the player behind a window can do beyond play, pause, seek and volume, as the player
/// said. Everything is offered until it says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MediaAbilities {
    /// Playing at another speed.
    pub speed: ControlOffer,
    /// Choosing among audio and subtitle tracks.
    pub tracks: ControlOffer,
    /// Chapters.
    pub chapters: ControlOffer,
    /// Stepping frame by frame.
    pub frame_step: ControlOffer,
}

/// What the stage needs from settings, and from the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaParams {
    /// How far a seek key jumps (setting `viewer.media.seek_step`, default 5 s).
    pub seek_step: MediaTime,
    /// What the player can do.
    pub abilities: MediaAbilities,
}

impl Default for MediaParams {
    fn default() -> Self {
        MediaParams {
            seek_step: MediaTime::from_secs(5),
            abilities: MediaAbilities::default(),
        }
    }
}
