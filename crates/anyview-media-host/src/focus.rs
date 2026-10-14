//! One source of sound at a time: when sessions of a hub share a speaker (several panes in one
//! window), the one that starts playing takes it and the others hold.

use super::hub::SessionId;
use anyview_media::Pace;
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

/// Which sessions sound, and which were paused for another.
///
/// What the person asked decides: a session they asked to play takes the sound at that moment,
/// whatever the players report afterwards. Reports are only a late echo of those asks, and
/// players may say `Playing` or `Paused` more than once, in any order, around one ask.
#[derive(Debug, Default)]
pub(super) struct Sounding {
    playing: Vec<SessionId>,
    /// Paused for another session and not asked to play since: a report of it playing is the
    /// echo of what it was doing, not a start, however late it comes or however often.
    yielded: Vec<SessionId>,
}

impl Sounding {
    /// The person asked session `id` to play or pause. Returns the sessions to pause: the others
    /// that were playing, when `id` was asked to play.
    pub(super) fn asked(&mut self, id: SessionId, pace: Pace) -> Vec<SessionId> {
        match pace {
            Pace::Playing => {
                self.yielded.retain(|yielded| *yielded != id);
                self.start(id)
            }
            Pace::Paused => {
                self.playing.retain(|playing| *playing != id);
                Vec::new()
            }
        }
    }

    /// Session `id` reports `status`. Returns the sessions to pause: the others that were playing,
    /// when `id` has just started on its own, which no ask came before.
    pub(super) fn heard(&mut self, id: SessionId, status: PlaybackStatus) -> Vec<SessionId> {
        match status {
            PlaybackStatus::Playing if self.yielded.contains(&id) => Vec::new(),
            PlaybackStatus::Playing => self.start(id),
            // A report of pausing may trail an ask to play, so only an ask takes a session out.
            PlaybackStatus::Paused => Vec::new(),
            PlaybackStatus::Stopped => {
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

    /// Session `id` sounds now: the others that were playing give way.
    fn start(&mut self, id: SessionId) -> Vec<SessionId> {
        if self.playing.contains(&id) {
            return Vec::new();
        }
        let others = std::mem::take(&mut self.playing);
        self.playing.push(id);
        self.yielded.extend(others.iter().copied());
        others
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlaybackStatus::{Paused, Playing, Stopped};

    /// What reaches the rule: the person's ask, or a player's report.
    enum Word {
        Ask(Pace),
        Report(PlaybackStatus),
    }

    #[test]
    fn the_session_the_person_started_last_keeps_the_sound() {
        let (a, b, c) = (SessionId(1), SessionId(2), SessionId(3));
        let mut sounding = Sounding::default();
        /// Name, who, what reached the rule, and who is paused for it.
        type Step = (&'static str, SessionId, Word, Vec<SessionId>);
        let steps: Vec<Step> = vec![
            (
                "the first to play pauses nobody",
                a,
                Word::Report(Playing),
                vec![],
            ),
            (
                "a repeated report of it pauses nobody",
                a,
                Word::Report(Playing),
                vec![],
            ),
            (
                "a second to play pauses the first",
                b,
                Word::Report(Playing),
                vec![a],
            ),
            (
                "the first's late report of playing is not a restart",
                a,
                Word::Report(Playing),
                vec![],
            ),
            (
                "the first says it has paused",
                a,
                Word::Report(Paused),
                vec![],
            ),
            (
                "a repeated report of the first playing, after that, is not a restart",
                a,
                Word::Report(Playing),
                vec![],
            ),
            (
                "and again after another pause",
                a,
                Word::Report(Paused),
                vec![],
            ),
            (
                "the second's report of pausing does not take it out",
                b,
                Word::Report(Paused),
                vec![],
            ),
            (
                "its playing again is no start",
                b,
                Word::Report(Playing),
                vec![],
            ),
            (
                "asking the first to play pauses the second",
                a,
                Word::Ask(Pace::Playing),
                vec![b],
            ),
            (
                "the second's late report of playing is not a restart",
                b,
                Word::Report(Playing),
                vec![],
            ),
            (
                "asking a third to play pauses the first",
                c,
                Word::Ask(Pace::Playing),
                vec![a],
            ),
            (
                "asking the third to play again pauses nobody",
                c,
                Word::Ask(Pace::Playing),
                vec![],
            ),
            (
                "a pause of the third is a pause",
                c,
                Word::Ask(Pace::Paused),
                vec![],
            ),
            (
                "so the first, asked to play, has nobody to pause",
                a,
                Word::Ask(Pace::Playing),
                vec![],
            ),
            (
                "one that stopped is not sounding",
                a,
                Word::Report(Stopped),
                vec![],
            ),
            (
                "so a session asked to play has nobody to pause",
                b,
                Word::Ask(Pace::Playing),
                vec![],
            ),
        ];
        for (name, who, word, want) in steps {
            let got = match word {
                Word::Ask(pace) => sounding.asked(who, pace),
                Word::Report(status) => sounding.heard(who, status),
            };
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn reports_that_trail_the_asks_do_not_pause_the_one_asked_last() {
        let (a, b) = (SessionId(1), SessionId(2));
        let mut sounding = Sounding::default();
        // Both are paused, then asked to play in turn, before any player has said a thing.
        assert!(sounding.asked(a, Pace::Paused).is_empty());
        assert!(sounding.asked(b, Pace::Paused).is_empty());
        assert!(sounding.asked(a, Pace::Playing).is_empty());
        assert_eq!(sounding.asked(b, Pace::Playing), vec![a]);
        // Then their reports arrive, in the order and number they please.
        for (who, status) in [
            (a, Playing),
            (b, Paused),
            (b, Playing),
            (a, Paused),
            (a, Playing),
            (b, Paused),
            (b, Playing),
            (a, Playing),
        ] {
            assert!(sounding.heard(who, status).is_empty());
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
