//! What the window knows of the player now: the position, the volume, the lists. The player's
//! news is applied here, and the stage, the capsule and the panel read it. It is data; the
//! machine decides what the position means.

use crate::{MediaAbilities, MediaNotice, PlayerEvent};
use anyview_core::{
    MediaChapter, MediaTime, MediaTrack, Percent, Resume, Speed, StreamKind, TimeRange,
    TrackChoice, TrackPlay, VideoPresence, Volume,
};

/// The panel's tabs once what the player cannot do is taken out: the tracks tab holds the tracks
/// and the speed, the contents tab the chapters.
pub(crate) fn tabs_offered(tabs: crate::PanelTabs, abilities: MediaAbilities) -> crate::PanelTabs {
    use crate::{ControlOffer, PanelTab};
    let tabs = match (abilities.tracks, abilities.speed) {
        (ControlOffer::Withheld, ControlOffer::Withheld) => tabs.without(PanelTab::Tracks),
        (ControlOffer::Offered, _) | (_, ControlOffer::Offered) => tabs,
    };
    match abilities.chapters {
        ControlOffer::Withheld => tabs.without(PanelTab::Contents),
        ControlOffer::Offered => tabs,
    }
}

/// Where a trim begins and ends, as the person marked them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrimMarks {
    /// The start mark.
    pub start: Option<MediaTime>,
    /// The end mark.
    pub end: Option<MediaTime>,
}

impl TrimMarks {
    /// The marks after `edge` is set at `at`. A start at or after the end, or an end at or
    /// before the start, drops the mark it would cross, as an editor does: the latest mark is
    /// the person's intent, and a cut is never silently the whole recording.
    pub fn marked(self, edge: crate::TrimEdge, at: MediaTime) -> TrimMarks {
        match edge {
            crate::TrimEdge::Start => TrimMarks {
                start: Some(at),
                end: self.end.filter(|end| *end > at),
            },
            crate::TrimEdge::End => TrimMarks {
                start: self.start.filter(|start| *start < at),
                end: Some(at),
            },
        }
    }

    /// Whether the person marked a start or an end.
    pub fn is_set(self) -> bool {
        self.start.is_some() || self.end.is_some()
    }

    /// The part of the recording the marks keep: all of it when none is set.
    pub fn range(self) -> TimeRange {
        let start = self.start.unwrap_or_default();
        TimeRange::new(start, self.end).unwrap_or(TimeRange::WHOLE)
    }
}

/// Where the person is in a recording and how they have it set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaPlace {
    /// The position.
    pub at: MediaTime,
    /// The output level.
    pub volume: Volume,
    /// The audio track.
    pub audio: TrackChoice,
    /// The subtitle track.
    pub subtitles: TrackChoice,
}

impl MediaPlace {
    /// The place as the store keeps it.
    pub fn resume(self) -> Resume {
        Resume::Media {
            at: self.at,
            volume: self.volume,
            audio: self.audio,
            subtitles: self.subtitles,
        }
    }

    /// The place a stored `resume` keeps, or `None` when it keeps another kind of place.
    pub fn of(resume: &Resume) -> Option<MediaPlace> {
        match resume {
            Resume::Media {
                at,
                volume,
                audio,
                subtitles,
            } => Some(MediaPlace {
                at: *at,
                volume: *volume,
                audio: *audio,
                subtitles: *subtitles,
            }),
            Resume::Raster { .. }
            | Resume::Pdf { .. }
            | Resume::Text { .. }
            | Resume::Book { .. }
            | Resume::Nothing => None,
        }
    }

    /// Whether `other` is the same place to the nearest second, with the same settings: a
    /// place that has only crept along is not worth keeping again.
    pub fn same_as(self, other: MediaPlace) -> bool {
        self.at.as_millis().abs_diff(other.at.as_millis()) < 1000
            && self.volume == other.volume
            && self.audio == other.audio
            && self.subtitles == other.subtitles
    }
}

/// The player's state as the window last heard it.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaLive {
    /// Where playback is, once the player has said: a player that has opened and is still
    /// finding a place it was told to start from has not.
    pub position: Option<MediaTime>,
    /// The output level.
    pub volume: Volume,
    /// The speed.
    pub speed: Speed,
    /// Every track.
    pub tracks: Vec<MediaTrack>,
    /// Every chapter.
    pub chapters: Vec<MediaChapter>,
    /// Whether a picture shows.
    pub picture: VideoPresence,
    /// How much of the recording the cache holds, 0 to 100. A file on disk is all there from the
    /// start, and the player says nothing about its cache.
    pub buffered: Percent,
    /// The trim marks.
    pub marks: TrimMarks,
    /// What the player can do.
    pub abilities: MediaAbilities,
}

impl Default for MediaLive {
    fn default() -> Self {
        MediaLive {
            position: None,
            volume: Volume::FULL,
            speed: Speed::NORMAL,
            tracks: Vec::new(),
            chapters: Vec::new(),
            picture: VideoPresence::Absent,
            buffered: Percent(100),
            marks: TrimMarks::default(),
            abilities: MediaAbilities::default(),
        }
    }
}

impl MediaLive {
    /// Take in one piece of news.
    pub fn apply(&mut self, notice: &MediaNotice) {
        match notice {
            MediaNotice::Position(at) => self.position = Some(*at),
            MediaNotice::Tracks(tracks) => self.tracks.clone_from(tracks),
            MediaNotice::Chapters(chapters) => self.chapters.clone_from(chapters),
            MediaNotice::Speed(speed) => self.speed = *speed,
            MediaNotice::Abilities(abilities) => self.abilities = *abilities,
            MediaNotice::Picture(presence) => self.picture = *presence,
            MediaNotice::Player(PlayerEvent::VolumeChanged(volume)) => self.volume = *volume,
            MediaNotice::Player(PlayerEvent::Buffering(level)) => self.buffered = *level,
            // A recording that has just opened has not said where playback is: what the player
            // reported while it opened was the start it is about to leave.
            MediaNotice::Player(PlayerEvent::Loaded { length: _ }) => self.position = None,
            MediaNotice::Player(
                PlayerEvent::LengthKnown(_)
                | PlayerEvent::Ended(_)
                | PlayerEvent::Playback(_)
                | PlayerEvent::SeekDone
                | PlayerEvent::TracksChanged,
            )
            | MediaNotice::Failed(_)
            | MediaNotice::Next
            | MediaNotice::Previous => {}
        }
    }

    /// The tracks of one kind.
    pub fn of_kind(&self, kind: StreamKind) -> impl Iterator<Item = &MediaTrack> {
        self.tracks.iter().filter(move |track| track.kind == kind)
    }

    /// Which track of `kind` plays: its number, `Off` when the kind has tracks and none plays,
    /// and the player's own choice when it has none.
    pub fn choice(&self, kind: StreamKind) -> TrackChoice {
        let mut tracks = self.of_kind(kind).peekable();
        if tracks.peek().is_none() {
            return TrackChoice::Auto;
        }
        tracks
            .find(|track| track.play == TrackPlay::Playing)
            .map_or(TrackChoice::Off, |track| TrackChoice::Track(track.id))
    }

    /// Where playback is and the settings, as the place to keep for next time; `None` until the
    /// player has said where playback is, so a place is never made of a start it has not left.
    pub fn place(&self) -> Option<MediaPlace> {
        Some(MediaPlace {
            at: self.position?,
            volume: self.volume,
            audio: self.choice(StreamKind::Audio),
            subtitles: self.choice(StreamKind::Subtitles),
        })
    }

    /// The chapter playing at `position`: the last that starts at or before it.
    pub fn chapter_at(&self, position: MediaTime) -> Option<usize> {
        self.chapters
            .iter()
            .rposition(|chapter| chapter.start <= position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TrimEdge::{End, Start};

    fn at(seconds: u64) -> MediaTime {
        MediaTime::from_secs(seconds)
    }

    #[test]
    fn a_mark_that_would_cross_the_other_drops_it_and_the_marks_stay_in_order() {
        // name, marks before (start, end), the edge set, where, marks after
        type Marks = (Option<u64>, Option<u64>);
        const CASES: &[(&str, Marks, crate::TrimEdge, u64, Marks)] = &[
            ("a first start", (None, None), Start, 10, (Some(10), None)),
            ("a first end", (None, None), End, 10, (None, Some(10))),
            (
                "an end after the start",
                (Some(10), None),
                End,
                20,
                (Some(10), Some(20)),
            ),
            (
                "an end before the start drops the start",
                (Some(40), None),
                End,
                20,
                (None, Some(20)),
            ),
            (
                "an end on the start drops the start",
                (Some(20), None),
                End,
                20,
                (None, Some(20)),
            ),
            (
                "a start before the end",
                (None, Some(40)),
                Start,
                20,
                (Some(20), Some(40)),
            ),
            (
                "a start after the end drops the end",
                (None, Some(20)),
                Start,
                40,
                (Some(40), None),
            ),
            (
                "a start on the end drops the end",
                (None, Some(20)),
                Start,
                20,
                (Some(20), None),
            ),
            (
                "moving the start inside the cut keeps the end",
                (Some(10), Some(40)),
                Start,
                30,
                (Some(30), Some(40)),
            ),
            (
                "moving the end past the start drops the start",
                (Some(30), Some(40)),
                End,
                5,
                (None, Some(5)),
            ),
        ];
        for (name, (start, end), edge, when, (want_start, want_end)) in CASES {
            let before = TrimMarks {
                start: start.map(at),
                end: end.map(at),
            };
            let after = before.marked(*edge, at(*when));
            assert_eq!(after.start, want_start.map(at), "{name}: start");
            assert_eq!(after.end, want_end.map(at), "{name}: end");
        }
    }
}
