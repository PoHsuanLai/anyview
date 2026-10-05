//! The session whose file is open: everything that needs a recording.

use super::opening::drain;
use super::report::Report;
use super::{Loaded, Session, convert, fault};
use crate::command::{Direction, Pace, ShotContent};
use crate::error::MediaError;
use crate::event::MediaEvent;
use anyview_core::{
    ChapterIndex, FilePath, MediaChapter, MediaLength, MediaTime, MediaTrack, Speed, StreamKind,
    TrackChoice, VideoPresence, Volume,
};
use mpv_wgpu_player as mpv;

impl Session<Loaded> {
    /// Drain mpv and draw. A failure of the player is an event, so a loop that polls need not
    /// stop for it.
    pub fn poll(&mut self) -> Report {
        match drain(&mut self.player) {
            Ok(report) => report,
            Err(reason) => Report {
                events: vec![MediaEvent::Failed(reason)],
                ..Report::quiet()
            },
        }
    }

    /// How long the recording runs, when it says. The child process reports it a moment after the
    /// file is loaded, so it may be known now and not have been when `Loaded` was said.
    pub fn length(&self) -> Option<MediaLength> {
        self.player.duration().map(convert::length)
    }

    /// Where playback is.
    pub fn position(&self) -> Option<MediaTime> {
        self.player.position().map(convert::time)
    }

    /// Whether the recording is advancing.
    pub fn pace(&self) -> Pace {
        convert::pace(self.player.playback())
    }

    /// Whether the picture is moving, a cover, or absent.
    pub fn presence(&self) -> VideoPresence {
        convert::presence(self.player.has_video())
    }

    /// Every track of the recording.
    pub fn tracks(&self) -> Vec<MediaTrack> {
        self.player.tracks().iter().map(convert::track_of).collect()
    }

    /// Every chapter of the recording.
    pub fn chapters(&self) -> Vec<MediaChapter> {
        self.player
            .chapters()
            .iter()
            .map(convert::chapter_of)
            .collect()
    }

    /// The output level.
    pub fn volume(&self) -> Volume {
        convert::volume_of(self.player.volume())
    }

    /// The speed.
    pub fn speed(&self) -> Speed {
        convert::speed_of(self.player.speed())
    }

    /// Play or hold.
    pub fn set_playback(&self, pace: Pace) -> Result<(), MediaError> {
        self.player
            .set_playback(convert::playback(pace))
            .map_err(fault)
    }

    /// Move to `to`, counted from the start.
    pub fn seek(&self, to: MediaTime) -> Result<(), MediaError> {
        let Some(seconds) = convert::seconds(to) else {
            return Err(MediaError::Player("the position is not finite".to_owned()));
        };
        self.player
            .seek(mpv::Seek::Absolute(seconds))
            .map_err(fault)
    }

    /// Set the output level.
    pub fn set_volume(&self, volume: Volume) -> Result<(), MediaError> {
        self.player
            .set_volume(convert::volume(volume))
            .map_err(fault)
    }

    /// Set the speed.
    pub fn set_speed(&self, speed: Speed) -> Result<(), MediaError> {
        match convert::speed(speed) {
            Some(speed) => self.player.set_speed(speed).map_err(fault),
            None => Err(MediaError::Player("the speed is not finite".to_owned())),
        }
    }

    /// Play at the next preset speed above or below the one playing; at either end nothing moves.
    pub fn step_speed(&self, direction: Direction) -> Result<(), MediaError> {
        let current = self.speed();
        let presets = Speed::PRESETS.iter().copied();
        let next = match direction {
            Direction::Forward => presets.clone().find(|preset| *preset > current),
            Direction::Backward => presets.rev().find(|preset| *preset < current),
        };
        next.map_or(Ok(()), |speed| self.set_speed(speed))
    }

    /// Play the track `choice` names among the tracks of `kind`.
    pub fn select_track(&self, kind: StreamKind, choice: TrackChoice) -> Result<(), MediaError> {
        self.player
            .select_track(convert::stream_kind(kind), convert::choice(choice))
            .map_err(fault)
    }

    /// Play the next track of `kind`. Audio and video wrap round their tracks; subtitles pass
    /// through "none" between the last track and the first.
    pub fn cycle_track(&self, kind: StreamKind) -> Result<(), MediaError> {
        let tracks = self.tracks();
        let of_kind: Vec<&MediaTrack> = tracks.iter().filter(|track| track.kind == kind).collect();
        let playing = of_kind
            .iter()
            .position(|track| track.play == anyview_core::TrackPlay::Playing);
        let next = match (playing, kind) {
            (None, _) => of_kind.first().map(|track| TrackChoice::Track(track.id)),
            (Some(at), StreamKind::Subtitles) if at + 1 == of_kind.len() => Some(TrackChoice::Off),
            (Some(at), StreamKind::Video | StreamKind::Audio | StreamKind::Subtitles) => of_kind
                .get(at + 1)
                .or(of_kind.first())
                .map(|track| TrackChoice::Track(track.id)),
        };
        match next {
            Some(choice) => self.select_track(kind, choice),
            None => Ok(()),
        }
    }

    /// Jump to the chapter after or before the one playing; at either end nothing moves.
    pub fn step_chapter(&self, direction: Direction) -> Result<(), MediaError> {
        let count = self.player.chapters().len();
        let current = self
            .player
            .chapter()
            .map_or(0, |index| index.get() as usize);
        let target = match direction {
            Direction::Forward => current + 1,
            Direction::Backward => current.saturating_sub(1),
        };
        match u32::try_from(target) {
            Ok(index) if target < count && target != current => {
                self.go_to_chapter(ChapterIndex(index))
            }
            Ok(_) | Err(_) => Ok(()),
        }
    }

    /// Jump to a chapter.
    pub fn go_to_chapter(&self, chapter: ChapterIndex) -> Result<(), MediaError> {
        self.player
            .set_chapter(mpv::ChapterIndex::new(chapter.0))
            .map_err(fault)
    }

    /// Show the next or previous frame; mpv holds afterwards.
    pub fn frame_step(&self, direction: Direction) -> Result<(), MediaError> {
        self.player
            .frame_step(convert::direction(direction))
            .map_err(fault)
    }

    /// Write the frame on screen to `to`, at the picture's own resolution; the extension picks
    /// the format.
    pub fn screenshot_to_file(
        &self,
        to: &FilePath,
        content: ShotContent,
    ) -> Result<(), MediaError> {
        self.player
            .screenshot_to_file(to.as_path(), convert::shot(content))
            .map_err(fault)
    }
}
