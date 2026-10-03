//! The media stage while the slider is dragged and after the end.

use super::event::{Pace, PlayerCommand};
use super::model::{AfterScrub, MediaIn, MediaParams, MediaStage};
use super::step::{Step, command, marked, notice, seek_target, setting};
use anyview_core::{MediaLength, MediaTime};

/// A scrub in progress: where it began, where it is, the recording's length, what to do after.
type Scrub = (MediaTime, MediaTime, MediaLength, AfterScrub);

/// Where a scrub ends up: resumed or held at `at`, with the seek that puts the player there.
fn finished(at: MediaTime, length: MediaLength, resume: AfterScrub) -> Step {
    let seek = command(PlayerCommand::Seek(at));
    match resume {
        AfterScrub::Play => (
            MediaStage::Playing { at, length },
            vec![seek, command(PlayerCommand::SetPlayback(Pace::Playing))],
        ),
        AfterScrub::Stay => (MediaStage::Paused { at, length }, vec![seek]),
    }
}

pub(super) fn scrubbing(this: MediaStage, scrub: Scrub, input: MediaIn) -> Step {
    let (from, to, length, resume) = scrub;
    match input {
        MediaIn::ScrubTo(at) => {
            let to = length.clamp(at);
            (
                MediaStage::Scrubbing {
                    from,
                    to,
                    length,
                    resume,
                },
                vec![command(PlayerCommand::Seek(to))],
            )
        }
        MediaIn::ScrubEnd => finished(to, length, resume),
        MediaIn::ScrubCancel => finished(from, length, resume),
        MediaIn::Failed(error) => (MediaStage::Failed(error), vec![]),
        MediaIn::Player(event) => (this, notice(event).into_iter().collect()),
        MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_) => (this, setting(input).into_iter().collect()),
        MediaIn::Mark(_) => (this, marked(input, to).into_iter().collect()),
        MediaIn::Position(_)
        | MediaIn::Toggle
        | MediaIn::SeekBack
        | MediaIn::SeekForward
        | MediaIn::SeekTo(_)
        | MediaIn::ScrubStart
        | MediaIn::FrameStep(_)
        | MediaIn::Restore { .. }
        | MediaIn::Elapsed => (this, vec![]),
    }
}

pub(super) fn ended(
    this: MediaStage,
    at: MediaTime,
    length: MediaLength,
    input: MediaIn,
    params: &MediaParams,
) -> Step {
    match input {
        MediaIn::Toggle => (
            MediaStage::Playing {
                at: MediaTime::default(),
                length,
            },
            vec![
                command(PlayerCommand::Seek(MediaTime::default())),
                command(PlayerCommand::SetPlayback(Pace::Playing)),
            ],
        ),
        MediaIn::SeekBack | MediaIn::SeekForward | MediaIn::SeekTo(_) => {
            match seek_target(at, length, input, params) {
                Some(to) => (
                    MediaStage::Paused { at: to, length },
                    vec![
                        command(PlayerCommand::Seek(to)),
                        command(PlayerCommand::SetPlayback(Pace::Paused)),
                    ],
                ),
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
        MediaIn::Position(_)
        | MediaIn::ScrubTo(_)
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::FrameStep(_)
        | MediaIn::Restore { .. }
        | MediaIn::Elapsed => (this, vec![]),
    }
}
