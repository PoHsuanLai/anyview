//! The media stage's transitions: the dispatch, opening and failed, and what every state shares.

use super::event::{EndReason, PlayerCommand, PlayerEvent};
use super::model::{MediaError, MediaIn, MediaOut, MediaParams, MediaStage};
use super::playing::{paused, playing};
use super::scrubbing::{ended, scrubbing};
use anyview_core::{MediaLength, MediaTime};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

pub(super) type Step = (MediaStage, Vec<MediaOut>);

impl Machine for MediaStage {
    type In = MediaIn;
    type Out = MediaOut;
    type Params = MediaParams;

    fn step(self, input: MediaIn, _at: Stamp, params: &MediaParams) -> Step {
        if let MediaIn::Player(PlayerEvent::LengthKnown(length)) = input {
            return (self.with_length(length), vec![]);
        }
        match self {
            MediaStage::Opening => opening(input),
            MediaStage::Playing { at, length } => playing(self, at, length, input, params),
            MediaStage::Paused { at, length } => paused(self, at, length, input, params),
            MediaStage::Scrubbing {
                from,
                to,
                length,
                resume,
            } => scrubbing(self, (from, to, length, resume), input),
            MediaStage::Ended { at, length } => ended(self, at, length, input, params),
            MediaStage::Failed(error) => failed(error, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            MediaStage::Opening
            | MediaStage::Playing { at: _, length: _ }
            | MediaStage::Paused { at: _, length: _ }
            | MediaStage::Scrubbing {
                from: _,
                to: _,
                length: _,
                resume: _,
            }
            | MediaStage::Ended { at: _, length: _ }
            | MediaStage::Failed(_) => None,
        }
    }
}

pub(super) fn command(command: PlayerCommand) -> MediaOut {
    MediaOut::Command(command)
}

/// A player notification that changes no state but is worth showing: the cache level, the
/// volume, a new track list.
pub(super) fn notice(event: PlayerEvent) -> Option<MediaOut> {
    match event {
        PlayerEvent::Buffering(level) => Some(MediaOut::Buffering(level)),
        PlayerEvent::VolumeChanged(volume) => Some(MediaOut::VolumeChanged(volume)),
        PlayerEvent::TracksChanged => Some(MediaOut::TracksChanged),
        PlayerEvent::Loaded { length: _ }
        | PlayerEvent::LengthKnown(_)
        | PlayerEvent::Ended(_)
        | PlayerEvent::Playback(_)
        | PlayerEvent::SeekDone => None,
    }
}

/// The state after the file stopped for `reason`, or `None` when the stop is not the end of
/// playback (the player quitting, or an entry redirected).
pub(super) fn stopped(reason: EndReason, at: MediaTime, length: MediaLength) -> Option<MediaStage> {
    match reason {
        EndReason::Eof => Some(MediaStage::Ended {
            at: length.0,
            length,
        }),
        EndReason::Stop => Some(MediaStage::Ended { at, length }),
        EndReason::Error => Some(MediaStage::Failed(MediaError::PlaybackFailed)),
        EndReason::Quit | EndReason::Redirect => None,
    }
}

/// Where a seek input lands, kept inside the recording; `None` for any other input.
pub(super) fn seek_target(
    at: MediaTime,
    length: MediaLength,
    input: MediaIn,
    params: &MediaParams,
) -> Option<MediaTime> {
    match input {
        MediaIn::SeekBack => Some(MediaTime(at.0.saturating_sub(params.seek_step.0))),
        MediaIn::SeekForward => {
            Some(length.clamp(MediaTime(at.0.saturating_add(params.seek_step.0))))
        }
        MediaIn::SeekTo(to) => Some(length.clamp(to)),
        MediaIn::Player(_)
        | MediaIn::Position(_)
        | MediaIn::Toggle
        | MediaIn::ScrubStart
        | MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::FrameStep(_)
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_)
        | MediaIn::Mark(_)
        | MediaIn::Restore { .. }
        | MediaIn::Failed(_)
        | MediaIn::Elapsed => None,
    }
}

/// The commands that do not depend on where playback is: volume, speed, tracks and chapters.
pub(super) fn setting(input: MediaIn) -> Option<MediaOut> {
    match input {
        MediaIn::SetVolume(volume) => Some(command(PlayerCommand::SetVolume(volume))),
        MediaIn::Select { kind, choice } => {
            Some(command(PlayerCommand::SelectTrack { kind, choice }))
        }
        MediaIn::SetSpeed(speed) => Some(command(PlayerCommand::SetSpeed(speed))),
        MediaIn::StepSpeed(direction) => Some(command(PlayerCommand::StepSpeed(direction))),
        MediaIn::CycleTrack(kind) => Some(command(PlayerCommand::CycleTrack(kind))),
        MediaIn::StepChapter(direction) => Some(command(PlayerCommand::StepChapter(direction))),
        MediaIn::GoToChapter(chapter) => Some(command(PlayerCommand::GoToChapter(chapter))),
        MediaIn::Player(_)
        | MediaIn::Position(_)
        | MediaIn::Toggle
        | MediaIn::SeekBack
        | MediaIn::SeekForward
        | MediaIn::SeekTo(_)
        | MediaIn::ScrubStart
        | MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::FrameStep(_)
        | MediaIn::Mark(_)
        | MediaIn::Restore { .. }
        | MediaIn::Failed(_)
        | MediaIn::Elapsed => None,
    }
}

/// The mark an input sets at `at`, if it is one.
pub(super) fn marked(input: MediaIn, at: MediaTime) -> Option<MediaOut> {
    match input {
        MediaIn::Mark(edge) => Some(MediaOut::Marked { edge, at }),
        MediaIn::Player(_)
        | MediaIn::Position(_)
        | MediaIn::Toggle
        | MediaIn::SeekBack
        | MediaIn::SeekForward
        | MediaIn::SeekTo(_)
        | MediaIn::ScrubStart
        | MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::FrameStep(_)
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_)
        | MediaIn::Restore { .. }
        | MediaIn::Failed(_)
        | MediaIn::Elapsed => None,
    }
}

/// The commands that put a person's settings back, in the order the player should hear them.
fn restoring(
    at: MediaTime,
    volume: anyview_core::Volume,
    audio: anyview_core::TrackChoice,
    subtitles: anyview_core::TrackChoice,
) -> Vec<MediaOut> {
    use super::event::TrackKind;
    let seek = (at.0 > 0).then(|| command(PlayerCommand::Seek(at)));
    seek.into_iter()
        .chain([
            command(PlayerCommand::SetVolume(volume)),
            command(PlayerCommand::SelectTrack {
                kind: TrackKind::Audio,
                choice: audio,
            }),
            command(PlayerCommand::SelectTrack {
                kind: TrackKind::Subtitles,
                choice: subtitles,
            }),
        ])
        .collect()
}

fn opening(input: MediaIn) -> Step {
    let stay = |outs: Vec<MediaOut>| (MediaStage::Opening, outs);
    match input {
        MediaIn::Player(PlayerEvent::Loaded { length }) => (
            MediaStage::Playing {
                at: MediaTime::default(),
                length,
            },
            vec![],
        ),
        MediaIn::Player(PlayerEvent::Ended(EndReason::Error)) => {
            (MediaStage::Failed(MediaError::OpenFailed), vec![])
        }
        MediaIn::Failed(error) => (MediaStage::Failed(error), vec![]),
        MediaIn::Player(event) => stay(notice(event).into_iter().collect()),
        MediaIn::Restore {
            at,
            volume,
            audio,
            subtitles,
        } => stay(restoring(at, volume, audio, subtitles)),
        MediaIn::SetVolume(_) | MediaIn::Select { kind: _, choice: _ } | MediaIn::SetSpeed(_) => {
            stay(setting(input).into_iter().collect())
        }
        MediaIn::Position(_)
        | MediaIn::Toggle
        | MediaIn::SeekBack
        | MediaIn::SeekForward
        | MediaIn::SeekTo(_)
        | MediaIn::ScrubStart
        | MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::FrameStep(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_)
        | MediaIn::Mark(_)
        | MediaIn::Elapsed => stay(vec![]),
    }
}

fn failed(error: MediaError, input: MediaIn) -> Step {
    match input {
        MediaIn::Player(_)
        | MediaIn::Position(_)
        | MediaIn::Toggle
        | MediaIn::SeekBack
        | MediaIn::SeekForward
        | MediaIn::SeekTo(_)
        | MediaIn::ScrubStart
        | MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::FrameStep(_)
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_)
        | MediaIn::Mark(_)
        | MediaIn::Restore { .. }
        | MediaIn::Failed(_)
        | MediaIn::Elapsed => (MediaStage::Failed(error), vec![]),
    }
}
