//! What the desktop is told of a player: its now-playing entry, built from the player's events.
//! Pure: an event goes in and the entry is as the desktop should see it, so the policy (what is
//! published at once and how often a moving position is) has a table of its own.

use anyview_core::{FilePath, MediaTags, MediaTime, Volume};
use anyview_media::{EndReason, MediaEvent, Pace};
use anyview_platform::{Ability, MediaState, PlaybackStatus, TrackSerial};

/// How many reports of a moving position pass between two published ones: the player reports
/// ten a second, and the desktop polls the position about once.
const POSITION_EVERY: u8 = 10;

/// Whether the entry changed in a way that is published now, only now and then, or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    /// Nothing the desktop shows moved.
    None,
    /// Only the position moved, and it is not yet time to publish it.
    Creep,
    /// Something the desktop shows changed: publish.
    Now,
}

/// The now-playing entry of one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    state: MediaState,
    creeping: u8,
}

impl Snapshot {
    /// The entry for `file`, before the player has said anything: titled from the file's tags, or
    /// its name, and not yet controllable.
    pub(super) fn new(file: &FilePath, tags: &MediaTags, serial: TrackSerial) -> Snapshot {
        let stem = file
            .file_name()
            .map(|name| name.as_str().to_owned())
            .filter(|name| !name.is_empty());
        Snapshot {
            state: MediaState {
                status: PlaybackStatus::Paused,
                track: serial,
                file: Some(file.clone()),
                title: tags.title.clone().or(stem),
                artist: tags.artist.clone(),
                album: tags.album.clone(),
                length: None,
                position: MediaTime::default(),
                volume: Volume::FULL,
                seek: Ability::Cannot,
                skip: Ability::Cannot,
            },
            creeping: 0,
        }
    }

    /// The entry as the desktop should show it.
    pub(super) fn state(&self) -> &MediaState {
        &self.state
    }

    /// Take in one event of the player.
    pub(super) fn apply(&mut self, event: &MediaEvent) -> Change {
        let before = self.state.clone();
        match event {
            MediaEvent::Loaded { length } => {
                self.state.length = *length;
                self.state.status = PlaybackStatus::Playing;
                self.state.seek = Ability::Can;
            }
            MediaEvent::Length(length) => self.state.length = Some(*length),
            MediaEvent::Playback(Pace::Playing) => self.state.status = PlaybackStatus::Playing,
            MediaEvent::Playback(Pace::Paused) => {
                if self.state.status != PlaybackStatus::Stopped {
                    self.state.status = PlaybackStatus::Paused;
                }
            }
            MediaEvent::Ended(EndReason::Eof | EndReason::Stop | EndReason::Error) => {
                self.state.status = PlaybackStatus::Stopped;
                self.state.position = self.state.length.map_or(self.state.position, |l| l.0);
            }
            MediaEvent::Ended(EndReason::Quit | EndReason::Redirect) => {}
            MediaEvent::Volume(volume) => self.state.volume = *volume,
            MediaEvent::Position(at) => {
                self.state.position = *at;
                // A file that plays again after it ended is playing, not stopped.
                if self.state.status == PlaybackStatus::Stopped {
                    self.state.status = PlaybackStatus::Paused;
                }
                return self.creep(&before);
            }
            MediaEvent::SeekDone => {}
            MediaEvent::Failed(_) => self.state.status = PlaybackStatus::Stopped,
            MediaEvent::Buffering(_)
            | MediaEvent::Tracks(_)
            | MediaEvent::Chapters(_)
            | MediaEvent::Speed(_)
            | MediaEvent::Picture(_)
            | MediaEvent::ShotSaved(_)
            | MediaEvent::ShotFailed { .. }
            | MediaEvent::Refused(_) => {}
        }
        match (self.state == before, event) {
            (true, MediaEvent::SeekDone) => Change::Now,
            (true, _) => Change::None,
            (false, _) => {
                self.creeping = 0;
                Change::Now
            }
        }
    }

    /// A position report: published every so often, and at once when it jumped (a seek).
    fn creep(&mut self, before: &MediaState) -> Change {
        let jumped = self.state.position.0.abs_diff(before.position.0) > 2_000_000;
        let status_moved = self.state.status != before.status;
        self.creeping = self.creeping.saturating_add(1);
        if jumped || status_moved || self.creeping >= POSITION_EVERY {
            self.creeping = 0;
            Change::Now
        } else {
            Change::Creep
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::MediaLength;

    fn file() -> FilePath {
        FilePath::new("/music/Song.flac").unwrap()
    }

    fn snapshot() -> Snapshot {
        Snapshot::new(&file(), &MediaTags::default(), TrackSerial(7))
    }

    fn secs(s: u64) -> MediaTime {
        MediaTime::from_secs(s)
    }

    #[test]
    fn a_new_entry_is_titled_from_the_tags_or_the_files_name() {
        let named = snapshot();
        assert_eq!(named.state().title.as_deref(), Some("Song.flac"));
        let tagged = Snapshot::new(
            &file(),
            &MediaTags {
                title: Some("A Song".to_owned()),
                artist: Some("A Band".to_owned()),
                album: None,
            },
            TrackSerial(1),
        );
        assert_eq!(tagged.state().title.as_deref(), Some("A Song"));
        assert_eq!(tagged.state().artist.as_deref(), Some("A Band"));
        assert_eq!(named.state().seek, Ability::Cannot, "nothing to seek yet");
    }

    /// Name, events in order, the status and position after, and the change each event made.
    #[test]
    fn the_entry_follows_the_player() {
        let loaded = MediaEvent::Loaded {
            length: Some(MediaLength(secs(100))),
        };
        let mut at = snapshot();
        assert_eq!(at.apply(&loaded), Change::Now);
        assert_eq!(at.state().status, PlaybackStatus::Playing);
        assert_eq!(at.state().length, Some(MediaLength(secs(100))));
        assert_eq!(at.state().seek, Ability::Can);

        assert_eq!(at.apply(&MediaEvent::Playback(Pace::Paused)), Change::Now);
        assert_eq!(at.state().status, PlaybackStatus::Paused);
        assert_eq!(at.apply(&MediaEvent::Playback(Pace::Paused)), Change::None);

        assert_eq!(at.apply(&MediaEvent::Volume(Volume::SILENT)), Change::Now);
        assert_eq!(at.state().volume, Volume::SILENT);

        assert_eq!(at.apply(&MediaEvent::Ended(EndReason::Eof)), Change::Now);
        assert_eq!(at.state().status, PlaybackStatus::Stopped);
        assert_eq!(
            at.state().position,
            secs(100),
            "an ended file is at its end"
        );
        assert_eq!(
            at.apply(&MediaEvent::Playback(Pace::Paused)),
            Change::None,
            "a stopped file stays stopped when told it is held"
        );
    }

    #[test]
    fn a_moving_position_is_published_now_and_then_and_a_jump_at_once() {
        let mut at = snapshot();
        at.apply(&MediaEvent::Loaded {
            length: Some(MediaLength(secs(600))),
        });
        let mut published = 0;
        let mut crept = 0;
        for tenth in 1..=30_u64 {
            match at.apply(&MediaEvent::Position(MediaTime::from_millis(tenth * 100))) {
                Change::Now => published += 1,
                Change::Creep => crept += 1,
                Change::None => {}
            }
        }
        assert_eq!(published, 3, "one report in ten reaches the desktop");
        assert_eq!(crept, 27);
        assert_eq!(
            at.apply(&MediaEvent::Position(secs(300))),
            Change::Now,
            "a seek is published at once"
        );
        assert_eq!(at.state().position, secs(300));
    }

    #[test]
    fn a_seek_that_lands_where_it_was_is_still_announced() {
        let mut at = snapshot();
        at.apply(&MediaEvent::Loaded { length: None });
        assert_eq!(at.apply(&MediaEvent::SeekDone), Change::Now);
    }

    #[test]
    fn a_file_that_plays_again_after_it_ended_is_not_stopped() {
        let mut at = snapshot();
        at.apply(&MediaEvent::Loaded {
            length: Some(MediaLength(secs(10))),
        });
        at.apply(&MediaEvent::Ended(EndReason::Eof));
        assert_eq!(at.apply(&MediaEvent::Position(secs(1))), Change::Now);
        assert_ne!(at.state().status, PlaybackStatus::Stopped);
    }

    #[test]
    fn a_failure_stops_the_entry() {
        let mut at = snapshot();
        assert_eq!(at.apply(&MediaEvent::Failed("no".to_owned())), Change::Now);
        assert_eq!(at.state().status, PlaybackStatus::Stopped);
    }
}
