//! The player's words as the stage machine's, both ways. The player (`anyview-media`) and the
//! machines (`anyview-ui`) are separate crates that name neither each other nor the same enum,
//! so the program, which links both, is where one becomes the other.

use anyview_core::{MediaLength, StreamKind};
use anyview_media::{Direction, EndReason, MediaCommand, MediaEvent, Pace};
use anyview_ui::{
    EndReason as UiEnd, MediaError, MediaNotice, Pace as UiPace, PlayerCommand, PlayerEvent,
    StepDirection, TrackKind,
};

/// Whether the file has opened, which decides what a failure of the player means: it could not
/// be opened, or it gave up part-way through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Opened {
    /// The player has not said the file is open.
    Not,
    /// It has.
    Yes,
}

pub(super) fn pace(pace: UiPace) -> Pace {
    match pace {
        UiPace::Playing => Pace::Playing,
        UiPace::Paused => Pace::Paused,
    }
}

fn ui_pace(pace: Pace) -> UiPace {
    match pace {
        Pace::Playing => UiPace::Playing,
        Pace::Paused => UiPace::Paused,
    }
}

fn direction(direction: StepDirection) -> Direction {
    match direction {
        StepDirection::Forward => Direction::Forward,
        StepDirection::Backward => Direction::Backward,
    }
}

fn kind(kind: TrackKind) -> StreamKind {
    match kind {
        TrackKind::Audio => StreamKind::Audio,
        TrackKind::Subtitles => StreamKind::Subtitles,
    }
}

fn ended(reason: EndReason) -> UiEnd {
    match reason {
        EndReason::Eof => UiEnd::Eof,
        EndReason::Stop => UiEnd::Stop,
        EndReason::Quit => UiEnd::Quit,
        EndReason::Redirect => UiEnd::Redirect,
        EndReason::Error => UiEnd::Error,
    }
}

/// The player's instruction for what the stage machine asked.
pub(super) fn command_of(command: PlayerCommand) -> MediaCommand {
    match command {
        PlayerCommand::SetPlayback(wanted) => MediaCommand::SetPlayback(pace(wanted)),
        PlayerCommand::Seek(to) => MediaCommand::Seek(to),
        PlayerCommand::SetVolume(volume) => MediaCommand::SetVolume(volume),
        PlayerCommand::SelectTrack {
            kind: which,
            choice,
        } => MediaCommand::SelectTrack {
            kind: kind(which),
            choice,
        },
        PlayerCommand::FrameStep(step) => MediaCommand::FrameStep(direction(step)),
        PlayerCommand::SetSpeed(speed) => MediaCommand::SetSpeed(speed),
        PlayerCommand::StepSpeed(step) => MediaCommand::StepSpeed(direction(step)),
        PlayerCommand::CycleTrack(which) => MediaCommand::CycleTrack(kind(which)),
        PlayerCommand::StepChapter(step) => MediaCommand::StepChapter(direction(step)),
        PlayerCommand::GoToChapter(chapter) => MediaCommand::GoToChapter(chapter),
    }
}

/// What the window is told of one event of the player; some events are for the program alone.
pub(super) fn notices_of(event: &MediaEvent, opened: Opened) -> Vec<MediaNotice> {
    match event {
        MediaEvent::Loaded { length } => vec![MediaNotice::Player(PlayerEvent::Loaded {
            length: length.unwrap_or(MediaLength::default()),
        })],
        MediaEvent::Ended(reason) => vec![MediaNotice::Player(PlayerEvent::Ended(ended(*reason)))],
        MediaEvent::Playback(now) => {
            vec![MediaNotice::Player(PlayerEvent::Playback(ui_pace(*now)))]
        }
        MediaEvent::SeekDone => vec![MediaNotice::Player(PlayerEvent::SeekDone)],
        MediaEvent::Buffering(level) => vec![MediaNotice::Player(PlayerEvent::Buffering(*level))],
        MediaEvent::Position(at) => vec![MediaNotice::Position(*at)],
        MediaEvent::Tracks(tracks) => vec![
            MediaNotice::Tracks(tracks.clone()),
            MediaNotice::Player(PlayerEvent::TracksChanged),
        ],
        MediaEvent::Chapters(chapters) => vec![MediaNotice::Chapters(chapters.clone())],
        MediaEvent::Volume(volume) => {
            vec![MediaNotice::Player(PlayerEvent::VolumeChanged(*volume))]
        }
        MediaEvent::Speed(speed) => vec![MediaNotice::Speed(*speed)],
        MediaEvent::Picture(presence) => vec![MediaNotice::Picture(*presence)],
        MediaEvent::Failed(_) => vec![MediaNotice::Failed(match opened {
            Opened::Not => MediaError::OpenFailed,
            Opened::Yes => MediaError::PlaybackFailed,
        })],
        MediaEvent::ShotSaved(_) | MediaEvent::ShotFailed { .. } | MediaEvent::Refused(_) => {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ChapterIndex, MediaTime, Percent, Speed, TrackChoice, Volume};

    #[test]
    fn every_instruction_of_the_stage_reaches_the_player_as_itself() {
        let cases: Vec<(&str, PlayerCommand, MediaCommand)> = vec![
            (
                "pause",
                PlayerCommand::SetPlayback(UiPace::Paused),
                MediaCommand::SetPlayback(Pace::Paused),
            ),
            (
                "seek",
                PlayerCommand::Seek(MediaTime::from_secs(3)),
                MediaCommand::Seek(MediaTime::from_secs(3)),
            ),
            (
                "volume",
                PlayerCommand::SetVolume(Volume::SILENT),
                MediaCommand::SetVolume(Volume::SILENT),
            ),
            (
                "subtitles off",
                PlayerCommand::SelectTrack {
                    kind: TrackKind::Subtitles,
                    choice: TrackChoice::Off,
                },
                MediaCommand::SelectTrack {
                    kind: StreamKind::Subtitles,
                    choice: TrackChoice::Off,
                },
            ),
            (
                "frame back",
                PlayerCommand::FrameStep(StepDirection::Backward),
                MediaCommand::FrameStep(Direction::Backward),
            ),
            (
                "speed",
                PlayerCommand::SetSpeed(Speed::NORMAL),
                MediaCommand::SetSpeed(Speed::NORMAL),
            ),
            (
                "slower",
                PlayerCommand::StepSpeed(StepDirection::Backward),
                MediaCommand::StepSpeed(Direction::Backward),
            ),
            (
                "next audio",
                PlayerCommand::CycleTrack(TrackKind::Audio),
                MediaCommand::CycleTrack(StreamKind::Audio),
            ),
            (
                "next chapter",
                PlayerCommand::StepChapter(StepDirection::Forward),
                MediaCommand::StepChapter(Direction::Forward),
            ),
            (
                "a chapter",
                PlayerCommand::GoToChapter(ChapterIndex(4)),
                MediaCommand::GoToChapter(ChapterIndex(4)),
            ),
        ];
        for (name, from, want) in cases {
            assert_eq!(command_of(from), want, "{name}");
        }
    }

    #[test]
    fn the_players_events_become_what_the_window_hears() {
        let length = MediaLength(MediaTime::from_secs(9));
        let cases: Vec<(&str, MediaEvent, Opened, Vec<MediaNotice>)> = vec![
            (
                "opened",
                MediaEvent::Loaded {
                    length: Some(length),
                },
                Opened::Not,
                vec![MediaNotice::Player(PlayerEvent::Loaded { length })],
            ),
            (
                "opened with no length",
                MediaEvent::Loaded { length: None },
                Opened::Not,
                vec![MediaNotice::Player(PlayerEvent::Loaded {
                    length: MediaLength::default(),
                })],
            ),
            (
                "the end",
                MediaEvent::Ended(EndReason::Eof),
                Opened::Yes,
                vec![MediaNotice::Player(PlayerEvent::Ended(UiEnd::Eof))],
            ),
            (
                "paused",
                MediaEvent::Playback(Pace::Paused),
                Opened::Yes,
                vec![MediaNotice::Player(PlayerEvent::Playback(UiPace::Paused))],
            ),
            (
                "the cache",
                MediaEvent::Buffering(Percent(40)),
                Opened::Yes,
                vec![MediaNotice::Player(PlayerEvent::Buffering(Percent(40)))],
            ),
            (
                "the position",
                MediaEvent::Position(MediaTime::from_secs(2)),
                Opened::Yes,
                vec![MediaNotice::Position(MediaTime::from_secs(2))],
            ),
            (
                "tracks are listed and announced",
                MediaEvent::Tracks(Vec::new()),
                Opened::Yes,
                vec![
                    MediaNotice::Tracks(Vec::new()),
                    MediaNotice::Player(PlayerEvent::TracksChanged),
                ],
            ),
            (
                "a failure before it opened is a failure to open",
                MediaEvent::Failed("no".to_owned()),
                Opened::Not,
                vec![MediaNotice::Failed(MediaError::OpenFailed)],
            ),
            (
                "a failure after it opened is a failure to play",
                MediaEvent::Failed("no".to_owned()),
                Opened::Yes,
                vec![MediaNotice::Failed(MediaError::PlaybackFailed)],
            ),
            (
                "a refusal is the program's",
                MediaEvent::Refused("no".to_owned()),
                Opened::Yes,
                Vec::new(),
            ),
        ];
        for (name, event, opened, want) in cases {
            assert_eq!(notices_of(&event, opened), want, "{name}");
        }
    }
}
