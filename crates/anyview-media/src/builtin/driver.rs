//! The built-in audio player as the media thread's driver: it decodes ahead of the sound card,
//! answers the window's instructions and reports where playback is, the same words the mpv driver
//! speaks.
//!
//! Playing is two threads and one queue. The sound card's thread drains a [`Pipe`] and wakes this
//! driver's owner as it does; the owner then calls [`MediaDriver::woken`], which decodes enough to
//! keep the queue full, and says the position at most ten times a second. The card's clock is the
//! recording's: a position is the frames the card has taken, so a pause, a stall and the end of the
//! recording are exact whatever the decoder is doing.

use super::convert::Conversion;
use super::output::SoundOutput;
use super::pipe::{Gate, Pipe};
use super::source::{Read, Source};
use crate::command::{MediaCommand, Pace};
use crate::error::MediaError;
use crate::event::{Abilities, Ability, EndReason, MediaEvent};
use crate::playing::{Continuation, Handled, MediaDriver};
use anyview_core::{
    FilePath, MediaLength, MediaTime, MediaTrack, Speed, StreamKind, TrackId, TrackPlay,
    VideoPresence, Volume,
};
use std::path::Path;
use std::sync::Arc;

/// How far a position moves before a new one is reported: ten reports a second at most.
const POSITION_STEP: u64 = 100_000;

/// How much sound is decoded ahead of the card, in tenths of a second.
const AHEAD_TENTHS: u64 = 5;

/// How many stretches of sound one turn may decode: a turn answers instructions in between.
const READS_PER_TURN: u32 = 64;

/// Whether the file has been announced to the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Told {
    /// `Loaded` has not been said: instructions wait.
    Not,
    /// It has.
    Yes,
}

/// Whether the end of the recording has been reported since playback last moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// Not reported.
    Unreported,
    /// Reported; playing or seeking makes it unreported again.
    Reported,
}

/// What the decoder is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decoding {
    /// More of the recording is to be read.
    Reading,
    /// All of it is queued.
    Finished,
    /// It failed: nothing more plays.
    Stopped,
}

/// Whether a file of a kind this player decodes can be played: its container and codec are ones
/// the decoders have, and its first stretch of sound decodes. Cheap: it reads the start of the file.
pub fn playable(file: &Path) -> Result<(), MediaError> {
    Source::open(file).map(drop)
}

/// One audio file playing through a sound output.
pub struct BuiltinDriver {
    source: Source,
    conversion: Conversion,
    pipe: Arc<Pipe>,
    output: Box<dyn SoundOutput>,
    decoded: Vec<f32>,
    converted: Vec<f32>,
    told: Told,
    queued: Vec<MediaCommand>,
    pace: Pace,
    volume: Volume,
    length: Option<MediaLength>,
    reported: Option<MediaTime>,
    ending: Ending,
    decoding: Decoding,
}

impl std::fmt::Debug for BuiltinDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltinDriver").finish_non_exhaustive()
    }
}

/// The factor a volume multiplies the sound by.
fn gain_of(volume: Volume) -> f32 {
    f32::from(volume.percent().0) / 100.0
}

impl BuiltinDriver {
    /// Open `file` and start `output` on it. The sound is held until the first `woken`, which
    /// says the file is loaded. `wake` is called from the sound card's thread whenever the owner
    /// should call `woken`, and once here: it must only wake.
    pub fn open(
        mut output: Box<dyn SoundOutput>,
        file: &FilePath,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<BuiltinDriver, MediaError> {
        let source = Source::open(file.as_path())?;
        let granted = output.negotiate(source.format())?;
        let wake = Arc::new(wake);
        let for_pipe = Arc::clone(&wake);
        let pipe = Pipe::new(granted, move || for_pipe());
        output.start(Arc::clone(&pipe))?;
        let mut driver = BuiltinDriver {
            conversion: Conversion::new(source.format(), granted),
            length: source.length(),
            source,
            pipe,
            output,
            decoded: Vec::new(),
            converted: Vec::new(),
            told: Told::Not,
            queued: Vec::new(),
            pace: Pace::Playing,
            volume: Volume::FULL,
            reported: None,
            ending: Ending::Unreported,
            decoding: Decoding::Reading,
        };
        if let Err(error) = driver.fill() {
            driver.stop();
            return Err(error);
        }
        wake();
        Ok(driver)
    }

    /// Decode until the card has enough queued, or this turn has used its share.
    fn fill(&mut self) -> Result<(), MediaError> {
        let format = self.pipe.format();
        let ahead = format.frames_a_second() * AHEAD_TENTHS / 10;
        let mut reads = 0;
        while self.decoding == Decoding::Reading
            && self.pipe.queued() < ahead
            && reads < READS_PER_TURN
        {
            reads += 1;
            self.decoded.clear();
            match self.source.read(&mut self.decoded)? {
                Read::Frames => {
                    self.converted.clear();
                    self.conversion.run(&self.decoded, &mut self.converted);
                    self.pipe.push(&self.converted);
                }
                Read::End => {
                    self.pipe.finish();
                    self.decoding = Decoding::Finished;
                }
            }
        }
        Ok(())
    }

    /// Nothing more plays.
    fn stop(&mut self) {
        self.decoding = Decoding::Stopped;
        self.pipe.set_gate(Gate::Shut);
        self.output.pause();
    }

    /// Where the card has reached.
    fn position(&self) -> MediaTime {
        let at = MediaTime(self.pipe.format().micros_in(self.pipe.position()));
        self.length.map_or(at, |length| length.clamp(at))
    }

    /// Say the file is loaded and what it has, run what waited for it, and let the sound through.
    fn announce(&mut self, events: &mut Vec<MediaEvent>) {
        self.told = Told::Yes;
        events.push(MediaEvent::Loaded {
            length: self.length,
        });
        events.push(MediaEvent::Tracks(vec![MediaTrack {
            id: TrackId(1),
            kind: StreamKind::Audio,
            title: None,
            language: None,
            codec: Some(self.source.codec().to_owned()),
            play: TrackPlay::Playing,
        }]));
        events.push(MediaEvent::Abilities(Abilities {
            speed: Ability::Cannot,
            tracks: Ability::Cannot,
            chapters: Ability::Cannot,
            frame_step: Ability::Cannot,
        }));
        events.push(MediaEvent::Chapters(Vec::new()));
        events.push(MediaEvent::Volume(self.volume));
        events.push(MediaEvent::Speed(Speed::NORMAL));
        events.push(MediaEvent::Picture(VideoPresence::Absent));
        for command in std::mem::take(&mut self.queued) {
            self.run(command, events);
        }
        self.apply_pace();
    }

    /// Open or shut the card's gate as the pace says.
    fn apply_pace(&mut self) {
        match (self.pace, self.decoding) {
            (Pace::Playing, Decoding::Reading | Decoding::Finished) => {
                self.pipe.set_gate(Gate::Open);
                self.output.resume();
            }
            (Pace::Paused, _) | (Pace::Playing, Decoding::Stopped) => {
                self.pipe.set_gate(Gate::Shut);
                self.output.pause();
            }
        }
    }

    /// Carry out an instruction that needs the file announced.
    fn run(&mut self, command: MediaCommand, events: &mut Vec<MediaEvent>) {
        if self.decoding == Decoding::Stopped {
            return;
        }
        match command {
            MediaCommand::SetPlayback(pace) => self.set_pace(pace, events),
            MediaCommand::Seek(to) => self.seek(to, events),
            MediaCommand::SetVolume(volume) => {
                self.volume = volume;
                self.pipe.set_gain(gain_of(volume));
                events.push(MediaEvent::Volume(volume));
            }
            // The built-in player plays at the recording's pace: the answer says so.
            MediaCommand::SetSpeed(_) | MediaCommand::StepSpeed(_) => {
                events.push(MediaEvent::Speed(Speed::NORMAL));
            }
            MediaCommand::Screenshot { to, .. } => events.push(MediaEvent::ShotFailed {
                to,
                reason: "an audio recording has no picture".to_owned(),
            }),
            MediaCommand::Slot(_)
            | MediaCommand::SelectTrack { .. }
            | MediaCommand::CycleTrack(_)
            | MediaCommand::StepChapter(_)
            | MediaCommand::GoToChapter(_)
            | MediaCommand::FrameStep(_)
            | MediaCommand::Close => {}
        }
    }

    fn set_pace(&mut self, pace: Pace, events: &mut Vec<MediaEvent>) {
        if pace == Pace::Playing && self.ending == Ending::Reported {
            // A recording that ended plays again from its start.
            self.seek(MediaTime::default(), events);
        }
        self.pace = pace;
        self.apply_pace();
        events.push(MediaEvent::Playback(pace));
    }

    fn seek(&mut self, to: MediaTime, events: &mut Vec<MediaEvent>) {
        let to = self.length.map_or(to, |length| length.clamp(to));
        self.pipe.set_gate(Gate::Shut);
        if let Err(error) = self.source.seek(to) {
            events.push(MediaEvent::Refused(error.to_string()));
            self.apply_pace();
            return;
        }
        self.pipe.restart_at(self.pipe.format().frames_in(to.0));
        self.conversion.reset();
        self.decoding = Decoding::Reading;
        self.ending = Ending::Unreported;
        let filled = self.fill();
        self.apply_pace();
        match filled {
            Ok(()) => {
                self.reported = Some(to);
                events.push(MediaEvent::SeekDone);
                events.push(MediaEvent::Position(to));
            }
            Err(error) => self.failed(&error, events),
        }
    }

    fn failed(&mut self, error: &MediaError, events: &mut Vec<MediaEvent>) {
        self.stop();
        events.push(MediaEvent::Failed(error.to_string()));
    }

    /// The end of the recording, once the card has played all of it.
    fn watch_end(&mut self, events: &mut Vec<MediaEvent>) {
        if self.ending == Ending::Reported || !self.pipe.is_played_out() {
            return;
        }
        self.ending = Ending::Reported;
        let at = self.length.map_or_else(
            || MediaTime(self.pipe.format().micros_in(self.pipe.position())),
            |l| l.0,
        );
        if self.length.is_none() {
            self.length = Some(MediaLength(at));
            events.push(MediaEvent::Length(MediaLength(at)));
        }
        self.reported = Some(at);
        events.push(MediaEvent::Position(at));
        self.pace = Pace::Paused;
        self.apply_pace();
        events.push(MediaEvent::Playback(Pace::Paused));
        events.push(MediaEvent::Ended(EndReason::Eof));
    }

    /// Report the position when it has moved enough since the last report.
    fn watch_position(&mut self, events: &mut Vec<MediaEvent>) {
        if self.ending == Ending::Reported {
            return;
        }
        let now = self.position();
        let moved = self
            .reported
            .is_none_or(|before| now.0.abs_diff(before.0) >= POSITION_STEP);
        if moved {
            self.reported = Some(now);
            events.push(MediaEvent::Position(now));
        }
    }
}

impl MediaDriver for BuiltinDriver {
    fn command(&mut self, command: MediaCommand) -> Handled {
        let mut events = Vec::new();
        let then = match (self.told, command) {
            (_, MediaCommand::Close) => Continuation::Close,
            (Told::Not, command) => {
                self.queued.push(command);
                Continuation::Keep
            }
            (Told::Yes, command) => {
                self.run(command, &mut events);
                Continuation::Keep
            }
        };
        Handled { events, then }
    }

    fn woken(&mut self) -> Vec<MediaEvent> {
        let mut events = Vec::new();
        if self.decoding == Decoding::Stopped {
            return events;
        }
        if let Some(reason) = self.pipe.take_failure() {
            self.failed(&MediaError::SoundOutput(reason), &mut events);
            return events;
        }
        if self.told == Told::Not {
            self.announce(&mut events);
        }
        if let Err(error) = self.fill() {
            self.failed(&error, &mut events);
            return events;
        }
        self.watch_end(&mut events);
        self.watch_position(&mut events);
        events
    }
}
