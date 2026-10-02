//! The player's words as the viewer's: tracks, chapters, volumes and positions.

use crate::command::{Direction, Pace, PictureSlot, ShotContent};
use crate::event::EndReason;
use anyview_core::{
    MediaChapter, MediaLength, MediaTime, MediaTrack, Percent, Speed, StreamKind, TrackChoice,
    TrackId, TrackPlay, VideoPresence, Volume,
};
use mpv_wgpu_player as mpv;

pub(super) fn stream_kind(kind: StreamKind) -> mpv::TrackKind {
    match kind {
        StreamKind::Video => mpv::TrackKind::Video,
        StreamKind::Audio => mpv::TrackKind::Audio,
        StreamKind::Subtitles => mpv::TrackKind::Subtitle,
    }
}

fn kind_of(kind: mpv::TrackKind) -> StreamKind {
    match kind {
        mpv::TrackKind::Video => StreamKind::Video,
        mpv::TrackKind::Audio => StreamKind::Audio,
        mpv::TrackKind::Subtitle => StreamKind::Subtitles,
    }
}

/// The track as the viewer holds it.
pub(super) fn track_of(track: &mpv::Track) -> MediaTrack {
    MediaTrack {
        id: TrackId(track.id.get()),
        kind: kind_of(track.kind),
        title: track.title.clone(),
        language: track.lang.clone(),
        codec: track.codec.clone(),
        play: match track.selected {
            mpv::TrackSelection::Selected => TrackPlay::Playing,
            mpv::TrackSelection::Unselected => TrackPlay::Idle,
        },
    }
}

/// The choice as the player takes it. A stored track number the player never assigns (0) is
/// the player's own choice.
pub(super) fn choice(choice: TrackChoice) -> mpv::TrackChoice {
    match choice {
        TrackChoice::Auto => mpv::TrackChoice::Auto,
        TrackChoice::Off => mpv::TrackChoice::Off,
        TrackChoice::Track(id) => {
            mpv::TrackId::new(id.0).map_or(mpv::TrackChoice::Auto, mpv::TrackChoice::Id)
        }
    }
}

pub(super) fn chapter_of(chapter: &mpv::Chapter) -> MediaChapter {
    MediaChapter {
        title: chapter.title.clone(),
        start: time(chapter.start),
    }
}

/// A position in seconds as a media time; a negative one (before the start) is the start.
pub(super) fn time(seconds: mpv::Finite) -> MediaTime {
    MediaTime((seconds.get().max(0.0) * 1_000_000.0).round() as u64)
}

/// A media time as the player's seconds. A time too large for an `f64` to hold exactly is
/// still finite, so the conversion always succeeds.
pub(super) fn seconds(time: MediaTime) -> Option<mpv::Finite> {
    mpv::Finite::new(time.0 as f64 / 1_000_000.0)
}

pub(super) fn length(seconds: mpv::Finite) -> MediaLength {
    MediaLength(time(seconds))
}

pub(super) fn volume_of(volume: mpv::Volume) -> Volume {
    Volume::clamped(Percent(u16::from(volume.percent())))
}

pub(super) fn volume(volume: Volume) -> mpv::Volume {
    mpv::Volume::new(u32::from(volume.percent().0))
}

pub(super) fn speed_of(speed: mpv::Speed) -> Speed {
    Speed::from_thousandths((speed.ratio() * 1000.0).round() as u32)
}

pub(super) fn speed(speed: Speed) -> Option<mpv::Speed> {
    mpv::Finite::new(f64::from(speed.thousandths()) / 1000.0).map(mpv::Speed::from_ratio)
}

pub(super) fn percent(percent: mpv::Percent) -> Percent {
    Percent(u16::from(percent.get()))
}

pub(super) fn pace(playback: mpv::Playback) -> Pace {
    match playback {
        mpv::Playback::Playing => Pace::Playing,
        mpv::Playback::Paused => Pace::Paused,
    }
}

pub(super) fn playback(pace: Pace) -> mpv::Playback {
    match pace {
        Pace::Playing => mpv::Playback::Playing,
        Pace::Paused => mpv::Playback::Paused,
    }
}

pub(super) fn end_reason(reason: mpv::EndReason) -> EndReason {
    match reason {
        mpv::EndReason::Eof => EndReason::Eof,
        mpv::EndReason::Stop => EndReason::Stop,
        mpv::EndReason::Quit => EndReason::Quit,
        mpv::EndReason::Redirect => EndReason::Redirect,
        mpv::EndReason::Error => EndReason::Error,
    }
}

pub(super) fn direction(direction: Direction) -> mpv::Direction {
    match direction {
        Direction::Forward => mpv::Direction::Forward,
        Direction::Backward => mpv::Direction::Backward,
    }
}

pub(super) fn shot(content: ShotContent) -> mpv::ScreenshotContent {
    match content {
        ShotContent::Video => mpv::ScreenshotContent::Video,
        ShotContent::Subtitles => mpv::ScreenshotContent::Subtitles,
    }
}

pub(super) fn presence(presence: mpv::VideoPresence) -> VideoPresence {
    match presence {
        mpv::VideoPresence::Absent => VideoPresence::Absent,
        mpv::VideoPresence::CoverArt => VideoPresence::CoverArt,
        mpv::VideoPresence::Present => VideoPresence::Present,
    }
}

pub(super) fn slot(slot: PictureSlot) -> mpv::Slot {
    match slot {
        PictureSlot::Empty => mpv::Slot::Empty,
        PictureSlot::Sized { width, height } => mpv::Slot::Sized(mpv::SlotSize { width, height }),
    }
}
