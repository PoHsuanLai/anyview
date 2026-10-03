//! What the desktop's controls mean to a player. Pure: a control and the entry it is pressed
//! against give the instructions, so each control has a row in a table.

use anyview_core::MediaTime;
use anyview_media::{MediaCommand, Pace};
use anyview_platform::{MediaControl, MediaState, PlaybackStatus, SeekDirection};

/// Where a player lives, which decides what Stop means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Home {
    /// In a window the person can see: Stop holds it at the start.
    Window,
    /// With no window: Stop ends it, since nothing else would.
    Background,
}

/// What a control asks of the program.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Order {
    /// Tell the player.
    Send(MediaCommand),
    /// End the session: the player and, with no window, its entry.
    End,
    /// End the viewer.
    Quit,
}

/// The instructions for `control` pressed against `state`.
pub(super) fn orders_for(control: MediaControl, state: &MediaState, home: Home) -> Vec<Order> {
    let send = |command| Order::Send(command);
    let play = || match state.status {
        // A file that ended plays again from the start.
        PlaybackStatus::Stopped => vec![
            send(MediaCommand::Seek(MediaTime::default())),
            send(MediaCommand::SetPlayback(Pace::Playing)),
        ],
        PlaybackStatus::Playing | PlaybackStatus::Paused => {
            vec![send(MediaCommand::SetPlayback(Pace::Playing))]
        }
    };
    match control {
        MediaControl::Play => play(),
        MediaControl::Pause => vec![send(MediaCommand::SetPlayback(Pace::Paused))],
        MediaControl::PlayPause => match state.status {
            PlaybackStatus::Playing => vec![send(MediaCommand::SetPlayback(Pace::Paused))],
            PlaybackStatus::Paused | PlaybackStatus::Stopped => play(),
        },
        MediaControl::Stop => match home {
            Home::Window => vec![
                send(MediaCommand::SetPlayback(Pace::Paused)),
                send(MediaCommand::Seek(MediaTime::default())),
            ],
            Home::Background => vec![Order::End],
        },
        MediaControl::SeekBy(direction, by) => {
            let to = match direction {
                SeekDirection::Forward => state.position.0.saturating_add(by.0),
                SeekDirection::Backward => state.position.0.saturating_sub(by.0),
            };
            let to = state.length.map_or(to, |length| to.min(length.0.0));
            vec![send(MediaCommand::Seek(MediaTime(to)))]
        }
        MediaControl::SeekTo(to) => vec![send(MediaCommand::Seek(to))],
        MediaControl::SetVolume(volume) => vec![send(MediaCommand::SetVolume(volume))],
        MediaControl::Quit => vec![Order::Quit],
        // Skipping walks the sequence, which belongs to a window; the entry says it cannot.
        MediaControl::Next | MediaControl::Previous | MediaControl::Raise => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{MediaLength, Percent, Volume};

    fn state(status: PlaybackStatus, secs: u64) -> MediaState {
        MediaState {
            status,
            position: MediaTime::from_secs(secs),
            length: Some(MediaLength(MediaTime::from_secs(100))),
            ..MediaState::stopped()
        }
    }

    fn secs(s: u64) -> MediaTime {
        MediaTime::from_secs(s)
    }

    #[test]
    fn each_control_means_something_to_the_player() {
        use MediaControl as C;
        let play = Order::Send(MediaCommand::SetPlayback(Pace::Playing));
        let pause = Order::Send(MediaCommand::SetPlayback(Pace::Paused));
        let seek = |s| Order::Send(MediaCommand::Seek(secs(s)));
        let cases: Vec<(&str, C, MediaState, Home, Vec<Order>)> = vec![
            (
                "play",
                C::Play,
                state(PlaybackStatus::Paused, 5),
                Home::Window,
                vec![play.clone()],
            ),
            (
                "play after the end starts over",
                C::Play,
                state(PlaybackStatus::Stopped, 100),
                Home::Window,
                vec![seek(0), play.clone()],
            ),
            (
                "pause",
                C::Pause,
                state(PlaybackStatus::Playing, 5),
                Home::Window,
                vec![pause.clone()],
            ),
            (
                "play-pause while playing pauses",
                C::PlayPause,
                state(PlaybackStatus::Playing, 5),
                Home::Window,
                vec![pause.clone()],
            ),
            (
                "play-pause while held plays",
                C::PlayPause,
                state(PlaybackStatus::Paused, 5),
                Home::Window,
                vec![play.clone()],
            ),
            (
                "play-pause after the end starts over",
                C::PlayPause,
                state(PlaybackStatus::Stopped, 100),
                Home::Background,
                vec![seek(0), play.clone()],
            ),
            (
                "stop in a window holds at the start",
                C::Stop,
                state(PlaybackStatus::Playing, 30),
                Home::Window,
                vec![pause.clone(), seek(0)],
            ),
            (
                "stop with no window ends the session",
                C::Stop,
                state(PlaybackStatus::Playing, 30),
                Home::Background,
                vec![Order::End],
            ),
            (
                "seek forward",
                C::SeekBy(SeekDirection::Forward, secs(10)),
                state(PlaybackStatus::Playing, 30),
                Home::Window,
                vec![seek(40)],
            ),
            (
                "seek forward stops at the end",
                C::SeekBy(SeekDirection::Forward, secs(10)),
                state(PlaybackStatus::Playing, 95),
                Home::Window,
                vec![seek(100)],
            ),
            (
                "seek back stops at the start",
                C::SeekBy(SeekDirection::Backward, secs(10)),
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![seek(0)],
            ),
            (
                "seek to",
                C::SeekTo(secs(50)),
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![seek(50)],
            ),
            (
                "volume",
                C::SetVolume(Volume::clamped(Percent(30))),
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![Order::Send(MediaCommand::SetVolume(Volume::clamped(
                    Percent(30),
                )))],
            ),
            (
                "next",
                C::Next,
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![],
            ),
            (
                "previous",
                C::Previous,
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![],
            ),
            (
                "raise",
                C::Raise,
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![],
            ),
            (
                "quit",
                C::Quit,
                state(PlaybackStatus::Playing, 4),
                Home::Window,
                vec![Order::Quit],
            ),
        ];
        for (name, control, state, home, want) in cases {
            assert_eq!(orders_for(control, &state, home), want, "{name}");
        }
    }
}
