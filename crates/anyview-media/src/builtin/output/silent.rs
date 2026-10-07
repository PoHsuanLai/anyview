//! An output that plays nothing but keeps time, for `ANYVIEW_AUDIO_OUTPUT=null`: the recording
//! advances and ends as if heard.

use super::{Pipe, SoundOutput, StreamFormat};
use crate::error::MediaError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// How often the silent output drains its pipe, and so how fine its clock is.
const PERIOD: Duration = Duration::from_millis(10);

/// Drains the pipe on a thread of its own at the pace the recording would be heard at, and ends
/// that thread when it is dropped or the pipe is.
#[derive(Debug, Default)]
pub struct SilentOutput {
    running: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl SoundOutput for SilentOutput {
    fn negotiate(&mut self, wanted: StreamFormat) -> Result<StreamFormat, MediaError> {
        Ok(wanted)
    }

    fn start(&mut self, pipe: Arc<Pipe>) -> Result<(), MediaError> {
        let format = pipe.format();
        let weak: Weak<Pipe> = Arc::downgrade(&pipe);
        let (running, stop) = (Arc::clone(&self.running), Arc::clone(&self.stop));
        running.store(true, Ordering::Relaxed);
        drop(pipe);
        std::thread::Builder::new()
            .name("anyview-silence".to_owned())
            .spawn(move || {
                let mut out = Vec::new();
                let mut last = Instant::now();
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(PERIOD);
                    let now = Instant::now();
                    let elapsed = now.duration_since(last);
                    last = now;
                    if !running.load(Ordering::Relaxed) {
                        continue;
                    }
                    let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
                    let frames = format.frames_in(micros) as usize;
                    out.clear();
                    out.resize(frames * format.width(), 0.0);
                    match weak.upgrade() {
                        Some(pipe) => pipe.fill(&mut out),
                        None => return,
                    }
                }
            })
            .map(drop)
            .map_err(|error| MediaError::SoundOutput(error.to_string()))
    }

    fn pause(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }

    fn resume(&mut self) {
        self.running.store(true, Ordering::Relaxed);
    }
}

impl Drop for SilentOutput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
