//! One source of sound at a time: when sessions of a hub share a speaker (several panes in one
//! window), the one that starts playing takes it and the others hold.

use super::hub::SessionId;
use anyview_platform::PlaybackStatus;

/// Whether the sessions of one hub may sound together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AudioFocus {
    /// Each session plays on its own: the viewer's windows, each with a player of its own.
    #[default]
    Shared,
    /// One session sounds at a time: a session that starts playing pauses the others.
    Exclusive,
}

/// Which sessions sound, and which were paused for another and have not yet said so.
#[derive(Debug, Default)]
pub(super) struct Sounding {
    playing: Vec<SessionId>,
    /// Paused by `heard`, whose pause has not come back yet: a report of the old status that is
    /// still on its way must not be taken for a session that started again.
    yielded: Vec<SessionId>,
}

impl Sounding {
    /// Session `id` reports `status`. Returns the sessions to pause: the others that were playing,
    /// when `id` has just started.
    pub(super) fn heard(&mut self, id: SessionId, status: PlaybackStatus) -> Vec<SessionId> {
        match status {
            PlaybackStatus::Playing => {
                if self.yielded.contains(&id) || self.playing.contains(&id) {
                    return Vec::new();
                }
                let others = std::mem::take(&mut self.playing);
                self.playing.push(id);
                self.yielded.extend(others.iter().copied());
                others
            }
            PlaybackStatus::Paused | PlaybackStatus::Stopped => {
                self.gone(id);
                Vec::new()
            }
        }
    }

    /// Session `id` is not sounding, or is gone.
    pub(super) fn gone(&mut self, id: SessionId) {
        self.playing.retain(|playing| *playing != id);
        self.yielded.retain(|yielded| *yielded != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_that_starts_playing_pauses_the_others_once() {
        let (a, b, c) = (SessionId(1), SessionId(2), SessionId(3));
        let mut sounding = Sounding::default();
        /// Name, who reports, the status, and who is paused for it.
        type Step = (&'static str, SessionId, PlaybackStatus, Vec<SessionId>);
        let steps: Vec<Step> = vec![
            (
                "the first to play pauses nobody",
                a,
                PlaybackStatus::Playing,
                vec![],
            ),
            (
                "a repeated report of it pauses nobody",
                a,
                PlaybackStatus::Playing,
                vec![],
            ),
            (
                "a second to play pauses the first",
                b,
                PlaybackStatus::Playing,
                vec![a],
            ),
            (
                "the first's late report of playing is not a restart",
                a,
                PlaybackStatus::Playing,
                vec![],
            ),
            (
                "the first says it has paused",
                a,
                PlaybackStatus::Paused,
                vec![],
            ),
            (
                "it starts again and pauses the second",
                a,
                PlaybackStatus::Playing,
                vec![b],
            ),
            (
                "a third pauses the first",
                c,
                PlaybackStatus::Playing,
                vec![a],
            ),
            (
                "one that stopped is not sounding",
                c,
                PlaybackStatus::Stopped,
                vec![],
            ),
            (
                "so the next to play has nobody to pause",
                b,
                PlaybackStatus::Paused,
                vec![],
            ),
        ];
        for (name, who, status, want) in steps {
            assert_eq!(sounding.heard(who, status), want, "{name}");
        }
    }

    #[test]
    fn a_session_that_is_gone_is_not_paused() {
        let (a, b) = (SessionId(1), SessionId(2));
        let mut sounding = Sounding::default();
        assert!(sounding.heard(a, PlaybackStatus::Playing).is_empty());
        sounding.gone(a);
        assert!(sounding.heard(b, PlaybackStatus::Playing).is_empty());
    }
}
