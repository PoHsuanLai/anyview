//! The player where mpv-wgpu is not built. Its host hands frames over a memfd ring and a
//! `SOCK_SEQPACKET` socket, which only Linux has, so off Linux no session can be made:
//! [`Session::new`] says so, in the words of a player that would not start, and every other call
//! is on a session that does not exist (it holds a value of an empty type). The types are the
//! ones the Linux build has, so the driver and the hosts above compile unchanged.

use super::{AudioDriver, Idle, Loaded, MpvHost, Opening, Report};
use crate::command::{Direction, Pace, PictureSlot, ShotContent};
use crate::error::MediaError;
use anyview_core::{
    ChapterIndex, FilePath, MediaChapter, MediaLength, MediaTime, MediaTrack, Speed, StreamKind,
    TrackChoice, VideoPresence, Volume,
};
use std::convert::Infallible;

/// A session that would not take the file, and why.
#[derive(Debug)]
pub struct Refused {
    /// The session, still idle.
    pub session: Box<Session<Idle>>,
    /// What went wrong.
    pub error: MediaError,
}

/// What a poll of an opening session found.
#[derive(Debug)]
pub enum Opened {
    /// mpv has not said yet.
    Waiting(Session<Opening>, Report),
    /// The file is open.
    Loaded(Session<Loaded>, Report),
    /// The file would not open.
    Failed(Session<Idle>, Report),
}

/// One mpv player, in the state `S`. None exists on this platform.
pub struct Session<S> {
    /// Why there is none to ask.
    never: Infallible,
    /// Which state it would be in.
    _state: S,
}

impl<S> std::fmt::Debug for Session<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl<S> Session<S> {
    /// Call `wake` whenever mpv wants attention.
    pub fn set_notify(&mut self, _wake: impl Fn() + Send + Sync + 'static) {
        match self.never {}
    }

    /// Where the picture goes.
    pub fn set_slot(&mut self, _slot: PictureSlot) -> Result<(), MediaError> {
        match self.never {}
    }

    /// The texture the player draws into, once it has drawn.
    pub fn picture(&self) -> Option<&wgpu::TextureView> {
        match self.never {}
    }
}

impl Session<Idle> {
    /// No player can be made here.
    pub fn new(
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _audio: AudioDriver,
        _host: &MpvHost,
    ) -> Result<Session<Idle>, MediaError> {
        Err(MediaError::PlayerStart(
            "the video player is not built for this platform".to_owned(),
        ))
    }

    /// Hand `file` to mpv.
    pub fn open(self, _file: &FilePath) -> Result<Session<Opening>, Refused> {
        match self.never {}
    }
}

impl Session<Opening> {
    /// Drain mpv and draw.
    pub fn poll(self) -> Opened {
        match self.never {}
    }
}

impl Session<Loaded> {
    /// Drain mpv and draw.
    pub fn poll(&mut self) -> Report {
        match self.never {}
    }

    /// How long the recording runs, when it says.
    pub fn length(&self) -> Option<MediaLength> {
        match self.never {}
    }

    /// Where playback is.
    pub fn position(&self) -> Option<MediaTime> {
        match self.never {}
    }

    /// Whether the recording is advancing.
    pub fn pace(&self) -> Pace {
        match self.never {}
    }

    /// Whether the picture is moving, a cover, or absent.
    pub fn presence(&self) -> VideoPresence {
        match self.never {}
    }

    /// Every track of the recording.
    pub fn tracks(&self) -> Vec<MediaTrack> {
        match self.never {}
    }

    /// Every chapter of the recording.
    pub fn chapters(&self) -> Vec<MediaChapter> {
        match self.never {}
    }

    /// The output level.
    pub fn volume(&self) -> Volume {
        match self.never {}
    }

    /// The speed.
    pub fn speed(&self) -> Speed {
        match self.never {}
    }

    /// Play or hold.
    pub fn set_playback(&self, _pace: Pace) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Move to `to`, counted from the start.
    pub fn seek(&self, _to: MediaTime) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Set the output level.
    pub fn set_volume(&self, _volume: Volume) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Set the speed.
    pub fn set_speed(&self, _speed: Speed) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Play at the next preset speed above or below the one playing.
    pub fn step_speed(&self, _direction: Direction) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Play the track `choice` names among the tracks of `kind`.
    pub fn select_track(&self, _kind: StreamKind, _choice: TrackChoice) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Play the next track of `kind`.
    pub fn cycle_track(&self, _kind: StreamKind) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Jump to the chapter after or before the one playing.
    pub fn step_chapter(&self, _direction: Direction) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Jump to a chapter.
    pub fn go_to_chapter(&self, _chapter: ChapterIndex) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Show the next or previous frame.
    pub fn frame_step(&self, _direction: Direction) -> Result<(), MediaError> {
        match self.never {}
    }

    /// Write the frame on screen to `to`.
    pub fn screenshot_to_file(
        &self,
        _to: &FilePath,
        _content: ShotContent,
    ) -> Result<(), MediaError> {
        match self.never {}
    }
}
