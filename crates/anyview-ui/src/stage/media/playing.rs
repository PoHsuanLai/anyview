//! The media stage while it plays and while it is held.

use super::event::{Pace, PlayerCommand, PlayerEvent};
use super::model::{AfterScrub, MediaIn, MediaParams, MediaStage};
use super::step::{Step, command, marked, notice, seek_target, setting, stopped};
use anyview_core::{MediaLength, MediaTime};

fn held(at: MediaTime, length: MediaLength) -> MediaStage {
    MediaStage::Paused { at, length }
}

fn advancing(at: MediaTime, length: MediaLength) -> MediaStage {
    MediaStage::Playing { at, length }
}

pub(super) fn playing(
    this: MediaStage,
    at: MediaTime,
    length: MediaLength,
    input: MediaIn,
    params: &MediaParams,
) -> Step {
    match input {
        MediaIn::Position(now) => (advancing(length.clamp(now), length), vec![]),
        MediaIn::Toggle => (
            held(at, length),
            vec![command(PlayerCommand::SetPlayback(Pace::Paused))],
        ),
        MediaIn::SeekBack | MediaIn::SeekForward | MediaIn::SeekTo(_) => {
            match seek_target(at, length, input, params) {
                Some(to) => (
                    advancing(to, length),
                    vec![command(PlayerCommand::Seek(to))],
                ),
                None => (this, vec![]),
            }
        }
        MediaIn::ScrubStart => (
            MediaStage::Scrubbing {
                from: at,
                to: at,
                length,
                resume: AfterScrub::Play,
            },
            vec![command(PlayerCommand::SetPlayback(Pace::Paused))],
        ),
        MediaIn::FrameStep(direction) => (
            held(at, length),
            vec![
                command(PlayerCommand::SetPlayback(Pace::Paused)),
                command(PlayerCommand::FrameStep(direction)),
            ],
        ),
        MediaIn::Player(PlayerEvent::Playback(Pace::Paused)) => (held(at, length), vec![]),
        MediaIn::Player(PlayerEvent::Ended(reason)) => match stopped(reason, at, length) {
            Some(state) => (state, vec![]),
            None => (this, vec![]),
        },
        MediaIn::Failed(error) => (MediaStage::Failed(error), vec![]),
        MediaIn::Player(event) => (this, notice(event).into_iter().collect()),
        MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_) => (this, setting(input).into_iter().collect()),
        MediaIn::Mark(_) => (this, marked(input, at).into_iter().collect()),
        MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::Restore { .. }
        | MediaIn::Elapsed => (this, vec![]),
    }
}

pub(super) fn paused(
    this: MediaStage,
    at: MediaTime,
    length: MediaLength,
    input: MediaIn,
    params: &MediaParams,
) -> Step {
    match input {
        MediaIn::Position(now) => (held(length.clamp(now), length), vec![]),
        MediaIn::Toggle => (
            advancing(at, length),
            vec![command(PlayerCommand::SetPlayback(Pace::Playing))],
        ),
        MediaIn::SeekBack | MediaIn::SeekForward | MediaIn::SeekTo(_) => {
            match seek_target(at, length, input, params) {
                Some(to) => (held(to, length), vec![command(PlayerCommand::Seek(to))]),
                None => (this, vec![]),
            }
        }
        MediaIn::ScrubStart => (
            MediaStage::Scrubbing {
                from: at,
                to: at,
                length,
                resume: AfterScrub::Stay,
            },
            vec![],
        ),
        MediaIn::FrameStep(direction) => (this, vec![command(PlayerCommand::FrameStep(direction))]),
        MediaIn::Player(PlayerEvent::Playback(Pace::Playing)) => (advancing(at, length), vec![]),
        MediaIn::Player(PlayerEvent::Ended(reason)) => match stopped(reason, at, length) {
            Some(state) => (state, vec![]),
            None => (this, vec![]),
        },
        MediaIn::Failed(error) => (MediaStage::Failed(error), vec![]),
        MediaIn::Player(event) => (this, notice(event).into_iter().collect()),
        MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_) => (this, setting(input).into_iter().collect()),
        MediaIn::Mark(_) => (this, marked(input, at).into_iter().collect()),
        MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::Restore { .. }
        | MediaIn::Elapsed => (this, vec![]),
    }
}
