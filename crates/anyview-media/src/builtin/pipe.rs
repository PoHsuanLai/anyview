//! The queue between the decoder and the sound card: the decoder pushes samples, the card's
//! callback drains them, and what the card has played is where the recording is.

use super::format::StreamFormat;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Whether the card is given sound or silence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Queued sound is played.
    Open,
    /// Silence is played and nothing is consumed (a pause).
    Shut,
}

/// Whether the decoder has more to push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Supply {
    /// It may push more.
    Open,
    /// The recording is decoded to its end: when the queue empties the recording has been played.
    Done,
}

struct State {
    samples: VecDeque<f32>,
    /// The frame of the recording the card had reached when `played` was zero.
    origin: u64,
    /// Frames the card has consumed since `origin`.
    played: u64,
    /// Frames consumed since the owner was last woken.
    since_wake: u64,
    gate: Gate,
    supply: Supply,
    gain: f32,
    failure: Option<String>,
}

/// The queue and the clock of one recording being played. Shared between the thread that decodes
/// and the thread the sound card calls from; a lock held for a copy of a few kilobytes is all they
/// contend on.
pub struct Pipe {
    state: Mutex<State>,
    format: StreamFormat,
    wake: Box<dyn Fn() + Send + Sync>,
}

impl std::fmt::Debug for Pipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipe")
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

impl Pipe {
    /// A shut, empty pipe of `format`. `wake` is called from the card's thread whenever the
    /// decoder should run: the queue is running low, or enough has played that the position moved.
    pub fn new(format: StreamFormat, wake: impl Fn() + Send + Sync + 'static) -> Arc<Pipe> {
        Arc::new(Pipe {
            state: Mutex::new(State {
                samples: VecDeque::new(),
                origin: 0,
                played: 0,
                since_wake: 0,
                gate: Gate::Shut,
                supply: Supply::Open,
                gain: 1.0,
                failure: None,
            }),
            format,
            wake: Box::new(wake),
        })
    }

    /// The format the card must be given.
    pub fn format(&self) -> StreamFormat {
        self.format
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Fill `out` (interleaved, in this pipe's format) for the card: queued sound at the gain while
    /// the gate is open, silence otherwise or when the queue runs dry. Called on the card's thread.
    pub fn fill(&self, out: &mut [f32]) {
        let wake = {
            let mut state = self.lock();
            let taken = match state.gate {
                Gate::Shut => 0,
                Gate::Open => out.len().min(state.samples.len()),
            };
            let gain = state.gain;
            for (slot, sample) in out.iter_mut().zip(state.samples.drain(..taken)) {
                *slot = (sample * gain).clamp(-1.0, 1.0);
            }
            out[taken..].fill(0.0);
            let frames = (taken / self.format.width()) as u64;
            state.played += frames;
            state.since_wake += frames;
            let rate = self.format.frames_a_second();
            let queued = (state.samples.len() / self.format.width()) as u64;
            let running_low = queued < rate / 4 && state.supply == Supply::Open;
            let moved = state.since_wake >= rate / 20;
            let finished = state.supply == Supply::Done && state.samples.is_empty();
            let wake = state.gate == Gate::Open && (running_low || moved || finished);
            if wake {
                state.since_wake = 0;
            }
            wake
        };
        if wake {
            (self.wake)();
        }
    }

    /// Queue `samples` (interleaved, in this pipe's format).
    pub fn push(&self, samples: &[f32]) {
        self.lock().samples.extend(samples.iter().copied());
    }

    /// How many frames are queued and not yet played.
    pub fn queued(&self) -> u64 {
        (self.lock().samples.len() / self.format.width()) as u64
    }

    /// Where the card has reached, in frames from the start of the recording.
    pub fn position(&self) -> u64 {
        let state = self.lock();
        state.origin + state.played
    }

    /// Drop everything queued and say the next sound pushed starts at `frame` of the recording.
    pub fn restart_at(&self, frame: u64) {
        let mut state = self.lock();
        state.samples.clear();
        state.origin = frame;
        state.played = 0;
        state.supply = Supply::Open;
    }

    /// Let sound through, or hold it.
    pub fn set_gate(&self, gate: Gate) {
        self.lock().gate = gate;
    }

    /// The factor every sample is multiplied by (1.0 is the recording's own level).
    pub fn set_gain(&self, gain: f32) {
        self.lock().gain = gain;
    }

    /// The decoder has pushed the last of the recording.
    pub fn finish(&self) {
        self.lock().supply = Supply::Done;
    }

    /// Whether the recording has been decoded to its end and the card has played all of it.
    pub fn is_played_out(&self) -> bool {
        let state = self.lock();
        state.supply == Supply::Done && state.samples.is_empty()
    }

    /// The card failed (it was unplugged, say): the decoder's owner hears of it on its next turn.
    pub fn fail(&self, reason: String) {
        self.lock().failure = Some(reason);
        (self.wake)();
    }

    /// Why the card failed, once.
    pub fn take_failure(&self) -> Option<String> {
        self.lock().failure.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn pipe() -> (Arc<Pipe>, Arc<AtomicU32>) {
        let woken = Arc::new(AtomicU32::new(0));
        let count = Arc::clone(&woken);
        let format = StreamFormat::new(100, 1).unwrap();
        let pipe = Pipe::new(format, move || {
            count.fetch_add(1, Ordering::Relaxed);
        });
        (pipe, woken)
    }

    #[test]
    fn a_shut_gate_plays_silence_and_the_clock_stands_still() {
        let (pipe, _) = pipe();
        pipe.push(&[0.5; 10]);
        let mut out = [1.0; 4];
        pipe.fill(&mut out);
        assert_eq!(out, [0.0; 4]);
        assert_eq!((pipe.position(), pipe.queued()), (0, 10));
    }

    #[test]
    fn an_open_gate_plays_what_is_queued_at_the_gain_and_the_clock_moves() {
        let (pipe, _) = pipe();
        pipe.push(&[0.5; 10]);
        pipe.set_gate(Gate::Open);
        pipe.set_gain(0.5);
        let mut out = [0.0; 4];
        pipe.fill(&mut out);
        assert_eq!(out, [0.25; 4]);
        assert_eq!((pipe.position(), pipe.queued()), (4, 6));
        pipe.set_gain(1.5);
        pipe.fill(&mut out);
        assert_eq!(out, [0.75; 4], "above the recording's own level");
    }

    #[test]
    fn the_gain_never_pushes_a_sample_past_full_scale() {
        let (pipe, _) = pipe();
        pipe.push(&[0.9, -0.9]);
        pipe.set_gate(Gate::Open);
        pipe.set_gain(1.5);
        let mut out = [0.0; 2];
        pipe.fill(&mut out);
        assert_eq!(out, [1.0, -1.0]);
    }

    #[test]
    fn running_dry_plays_silence_for_the_rest_and_counts_only_what_played() {
        let (pipe, _) = pipe();
        pipe.push(&[0.5; 3]);
        pipe.set_gate(Gate::Open);
        let mut out = [9.0; 5];
        pipe.fill(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.5, 0.0, 0.0]);
        assert_eq!(pipe.position(), 3);
    }

    #[test]
    fn restarting_drops_the_queue_and_moves_the_clock() {
        let (pipe, _) = pipe();
        pipe.push(&[0.5; 10]);
        pipe.set_gate(Gate::Open);
        pipe.fill(&mut [0.0; 2]);
        pipe.restart_at(500);
        assert_eq!((pipe.position(), pipe.queued()), (500, 0));
        pipe.push(&[0.5; 2]);
        pipe.fill(&mut [0.0; 2]);
        assert_eq!(pipe.position(), 502);
    }

    #[test]
    fn the_recording_is_played_out_when_it_is_finished_and_the_queue_is_empty() {
        let (pipe, _) = pipe();
        pipe.push(&[0.5; 2]);
        pipe.finish();
        assert!(!pipe.is_played_out(), "sound is still queued");
        pipe.set_gate(Gate::Open);
        pipe.fill(&mut [0.0; 2]);
        assert!(pipe.is_played_out());
        pipe.restart_at(0);
        assert!(!pipe.is_played_out(), "a restart reopens the supply");
    }

    #[test]
    fn the_owner_is_woken_when_the_queue_runs_low_and_as_time_passes_but_not_while_shut() {
        let (pipe, woken) = pipe();
        pipe.push(&[0.5; 1000]);
        let mut out = [0.0; 2];
        pipe.fill(&mut out);
        assert_eq!(woken.load(Ordering::Relaxed), 0, "shut");
        pipe.set_gate(Gate::Open);
        pipe.fill(&mut out);
        assert_eq!(
            woken.load(Ordering::Relaxed),
            0,
            "full and no time has passed"
        );
        // 100 frames a second: 5 frames are a twentieth of a second.
        pipe.fill(&mut [0.0; 3]);
        assert_eq!(
            woken.load(Ordering::Relaxed),
            1,
            "a twentieth of a second played"
        );
        pipe.restart_at(0);
        pipe.push(&[0.5; 10]);
        pipe.fill(&mut [0.0; 1]);
        assert_eq!(woken.load(Ordering::Relaxed), 2, "running low");
    }

    #[test]
    fn a_failure_is_kept_for_the_owner_and_woken_once() {
        let (pipe, woken) = pipe();
        pipe.fail("unplugged".to_owned());
        assert_eq!(woken.load(Ordering::Relaxed), 1);
        assert_eq!(pipe.take_failure().as_deref(), Some("unplugged"));
        assert_eq!(pipe.take_failure(), None);
    }
}
