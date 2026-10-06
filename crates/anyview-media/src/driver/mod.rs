//! The media thread's object: one session, the commands it is sent and the events it makes. The
//! binary's actor owns a [`Driver`] and calls [`Driver::command`] for what the window sends and
//! [`Driver::woken`] when mpv asks for attention; nothing here spawns a thread.
//!
//! The driver is what turns mpv's few signals into the viewer's richer ones: a position at most
//! ten times a second, the end of a file that mpv holds open instead of ending (`keep-open`),
//! the lists a loaded recording has, and a texture announced once and again whenever the slot
//! changes size. Instructions that arrive before the file is open wait and run, in order, the
//! moment it is.

mod held;
mod sink;

pub use sink::FrameSink;

use crate::command::{MediaCommand, Pace, PictureSlot};
use crate::error::MediaError;
use crate::event::{EndReason, MediaEvent};
use crate::playing::{Continuation, Handled, MediaDriver};
use crate::session::{AudioDriver, Frame, Idle, MpvHost, Report, Session};
use anyview_core::{FilePath, MediaTime};
use held::{Held, Moved};

/// How far a position moves before a new one is reported: ten reports a second at most.
const POSITION_STEP: u64 = 100_000;

/// How near the end a paused player counts as having ended: mpv holds a finished file open
/// paused, a frame or so before its length.
const END_SLACK: u64 = 250_000;

/// Whether a seek has been asked for and the player has not yet said it landed. The position
/// the player reports in between is where it was, not where it is going, so none is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seeking {
    /// No seek is in flight.
    Settled,
    /// A seek was asked for; `SeekDone` ends it.
    Waiting,
}

/// Whether the recording's length has been told to the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Told {
    /// Not yet: the file is not loaded, or it was loaded with no length.
    Not,
    /// Said, in `Loaded` or in `Length`.
    Yes,
}

/// Whether the end of the recording has been reported since playback last moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// Not reported: a pause at the end is the end.
    Unreported,
    /// Reported; playing or seeking makes it unreported again.
    Reported,
}

/// Whether the texture the player draws into has been shown to the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Announced {
    /// The sink has been told of the current texture, or there is none.
    Yes,
    /// The slot changed or the session is new: the next frame's texture must be announced.
    Not,
}

/// One player and the state around it.
pub struct Driver {
    /// `None` only while a move between states is under way.
    held: Option<Held>,
    sink: Box<dyn FrameSink>,
    queued: Vec<MediaCommand>,
    announced: Announced,
    slot: PictureSlot,
    reported: Option<MediaTime>,
    seeking: Seeking,
    ending: Ending,
    length: Told,
}

impl std::fmt::Debug for Driver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Driver").finish_non_exhaustive()
    }
}

impl Driver {
    /// A player on `device` and `queue` playing `file` with its sound on `audio`, mpv run as `host`
    /// says. `wake` is
    /// called, from an mpv thread, whenever the player wants `woken` called; it must only wake.
    /// It is also called once here, after the file is handed over: mpv's first events may be
    /// queued already, and it calls back only when its queue goes from empty to not, so a player
    /// with no picture to draw (an audio file) would otherwise never be heard from.
    pub fn open(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        audio: AudioDriver,
        host: &MpvHost,
        file: &FilePath,
        sink: Box<dyn FrameSink>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Driver, MediaError> {
        let mut idle: Session<Idle> = Session::new(device, queue, audio, host)?;
        let wake = std::sync::Arc::new(wake);
        let for_player = std::sync::Arc::clone(&wake);
        idle.set_notify(move || for_player());
        let opening = idle.open(file).map_err(|refused| refused.error)?;
        wake();
        Ok(Driver {
            held: Some(Held::Opening(opening)),
            sink,
            queued: Vec::new(),
            announced: Announced::Not,
            slot: PictureSlot::Empty,
            reported: None,
            seeking: Seeking::Settled,
            ending: Ending::Unreported,
            length: Told::Not,
        })
    }

    fn apply(&mut self, command: MediaCommand, events: &mut Vec<MediaEvent>) -> Continuation {
        match (&mut self.held, command) {
            (_, MediaCommand::Close) => Continuation::Close,
            (Some(held), MediaCommand::Slot(slot)) => {
                if let Err(error) = held.set_slot(slot) {
                    events.push(MediaEvent::Refused(error.to_string()));
                }
                if slot != self.slot {
                    self.slot = slot;
                    self.announced = Announced::Not;
                }
                Continuation::Keep
            }
            (Some(Held::Loaded(_)), command) => {
                self.run(command, events);
                Continuation::Keep
            }
            (Some(Held::Idle(_) | Held::Opening(_)) | None, command) => {
                self.queued.push(command);
                Continuation::Keep
            }
        }
    }

    /// Carry out an instruction that needs a loaded recording.
    fn run(&mut self, command: MediaCommand, events: &mut Vec<MediaEvent>) {
        let Some(Held::Loaded(session)) = &self.held else {
            return;
        };
        let refused = |result: Result<(), MediaError>, events: &mut Vec<MediaEvent>| {
            if let Err(error) = result {
                events.push(MediaEvent::Refused(error.to_string()));
            }
        };
        match command {
            // mpv says nothing of a pause the host asked for (its getter changes at once, so the
            // property's echo is no change), and the window and the desktop's entry must hear it.
            MediaCommand::SetPlayback(pace) => match session.set_playback(pace) {
                Ok(()) => {
                    events.push(MediaEvent::Playback(session.pace()));
                    match pace {
                        Pace::Playing => self.ending = Ending::Unreported,
                        Pace::Paused => {}
                    }
                }
                Err(error) => events.push(MediaEvent::Refused(error.to_string())),
            },
            MediaCommand::Seek(to) => match session.seek(to) {
                Ok(()) => {
                    self.reported = None;
                    self.seeking = Seeking::Waiting;
                    self.ending = Ending::Unreported;
                }
                Err(error) => events.push(MediaEvent::Refused(error.to_string())),
            },
            MediaCommand::SetVolume(volume) => refused(session.set_volume(volume), events),
            MediaCommand::SetSpeed(speed) => {
                refused(session.set_speed(speed), events);
                events.push(MediaEvent::Speed(session.speed()));
            }
            MediaCommand::StepSpeed(direction) => {
                refused(session.step_speed(direction), events);
                events.push(MediaEvent::Speed(session.speed()));
            }
            MediaCommand::SelectTrack { kind, choice } => {
                refused(session.select_track(kind, choice), events);
            }
            MediaCommand::CycleTrack(kind) => refused(session.cycle_track(kind), events),
            MediaCommand::StepChapter(direction) => {
                refused(session.step_chapter(direction), events);
            }
            MediaCommand::GoToChapter(chapter) => refused(session.go_to_chapter(chapter), events),
            MediaCommand::FrameStep(direction) => refused(session.frame_step(direction), events),
            MediaCommand::Screenshot { to, content } => {
                match session.screenshot_to_file(&to, content) {
                    Ok(()) => events.push(MediaEvent::ShotSaved(to)),
                    Err(error) => events.push(MediaEvent::ShotFailed {
                        to,
                        reason: error.to_string(),
                    }),
                }
            }
            MediaCommand::Slot(_) | MediaCommand::Close => {}
        }
    }

    /// The file just opened: say what a loaded recording has, then run what waited for it.
    fn loaded(&mut self, events: &mut Vec<MediaEvent>) {
        if let Some(Held::Loaded(session)) = &self.held {
            events.push(MediaEvent::Tracks(session.tracks()));
            events.push(MediaEvent::Chapters(session.chapters()));
            events.push(MediaEvent::Volume(session.volume()));
            events.push(MediaEvent::Speed(session.speed()));
            events.push(MediaEvent::Picture(session.presence()));
        }
        for command in std::mem::take(&mut self.queued) {
            self.run(command, events);
        }
    }

    /// Hand a drawn frame to the sink, announcing the texture first when it is new.
    fn present(&mut self, frame: Frame) {
        let Some(held) = &self.held else {
            return;
        };
        match (held.picture(), self.announced, frame) {
            (Some(view), Announced::Not, _) => {
                self.sink.texture(view);
                self.announced = Announced::Yes;
            }
            (Some(_), Announced::Yes, Frame::Rewritten) => self.sink.frame(),
            (Some(_), Announced::Yes, Frame::Unchanged) => {}
            (None, _, Frame::Rewritten) => self.sink.cleared(),
            (None, _, Frame::Unchanged) => {}
        }
    }

    /// Copy a poll's events out, adding the end of a file that mpv held open. mpv pauses a finished
    /// file a frame before its length, and the child process reports that last position after the
    /// pause, so the end is looked for once the whole poll is in.
    fn absorb(&mut self, report: Report, events: &mut Vec<MediaEvent>) {
        for event in report.events {
            events.push(event);
            match events.last() {
                Some(MediaEvent::SeekDone) => {
                    self.reported = None;
                    self.seeking = Seeking::Settled;
                    self.ending = Ending::Unreported;
                }
                Some(MediaEvent::Ended(_)) => self.seeking = Seeking::Settled,
                Some(MediaEvent::Playback(Pace::Playing)) => self.ending = Ending::Unreported,
                Some(MediaEvent::Loaded { length: Some(_) } | MediaEvent::Length(_)) => {
                    self.length = Told::Yes;
                }
                Some(
                    MediaEvent::Playback(Pace::Paused)
                    | MediaEvent::Loaded { length: None }
                    | MediaEvent::Buffering(_)
                    | MediaEvent::Position(_)
                    | MediaEvent::Tracks(_)
                    | MediaEvent::Chapters(_)
                    | MediaEvent::Volume(_)
                    | MediaEvent::Speed(_)
                    | MediaEvent::Picture(_)
                    | MediaEvent::ShotSaved(_)
                    | MediaEvent::ShotFailed { .. }
                    | MediaEvent::Failed(_)
                    | MediaEvent::Refused(_),
                )
                | None => {}
            }
        }
        if self.ending == Ending::Unreported && self.held_at_the_end() {
            self.ending = Ending::Reported;
            events.push(MediaEvent::Ended(EndReason::Eof));
        }
    }

    /// Whether a paused player is within a frame or so of the length.
    fn held_at_the_end(&self) -> bool {
        let Some(Held::Loaded(session)) = &self.held else {
            return false;
        };
        match (session.pace(), session.position(), session.length()) {
            (Pace::Paused, Some(at), Some(length)) => at.0.saturating_add(END_SLACK) >= length.0.0,
            (Pace::Playing, _, _) | (_, None, _) | (_, _, None) => false,
        }
    }

    /// Say the length once the recording gives it, when `Loaded` could not.
    fn watch_length(&mut self, events: &mut Vec<MediaEvent>) {
        let Some(Held::Loaded(session)) = &self.held else {
            return;
        };
        if self.length == Told::Yes {
            return;
        }
        if let Some(length) = session.length() {
            self.length = Told::Yes;
            events.push(MediaEvent::Length(length));
        }
    }

    /// Report the position when it has moved enough since the last report.
    fn watch_position(&mut self, events: &mut Vec<MediaEvent>) {
        let Some(Held::Loaded(session)) = &self.held else {
            return;
        };
        // A file held at its end says nothing more of where it is: the child process reports the
        // last position after the pause, and a position after the end would read as playing again.
        if self.seeking == Seeking::Waiting || self.ending == Ending::Reported {
            return;
        }
        let Some(now) = session.position() else {
            return;
        };
        let moved = self
            .reported
            .is_none_or(|before| now.0.abs_diff(before.0) >= POSITION_STEP);
        if moved {
            self.reported = Some(now);
            events.push(MediaEvent::Position(now));
        }
    }
}

impl MediaDriver for Driver {
    fn command(&mut self, command: MediaCommand) -> Handled {
        let mut events = Vec::new();
        let then = self.apply(command, &mut events);
        Handled { events, then }
    }

    fn woken(&mut self) -> Vec<MediaEvent> {
        let Some(held) = self.held.take() else {
            return Vec::new();
        };
        let (held, report, moved) = held.poll();
        self.held = Some(held);
        let mut events = Vec::new();
        self.present(report.frame);
        self.absorb(report, &mut events);
        if moved == Moved::ToLoaded {
            self.loaded(&mut events);
        }
        self.watch_length(&mut events);
        self.watch_position(&mut events);
        events
    }
}
