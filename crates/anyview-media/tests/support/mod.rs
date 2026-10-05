//! A headless device, the fixtures, and a loop that drives a driver the way the media thread does.
#![cfg(feature = "player")]
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{FilePath, MediaTrack};
use anyview_media::{
    AudioDriver, Device, Driver, FrameSink, MediaCommand, MediaEvent, MpvHost, Queue, TextureView,
    headless_device,
};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

pub const TIMEOUT: Duration = Duration::from_secs(20);

pub fn fixture(name: &str) -> FilePath {
    FilePath::new(
        std::fs::canonicalize(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

/// The programs a test plays with: the stock `mpv` (`MPV_WGPU_MPV`, else the first on the search
/// path) and the C plugin built from mpv-wgpu (`MPV_WGPU_CPLUGIN`). `None` with a note when
/// either is missing, so a machine without them skips the player's tests rather than fails them.
pub fn mpv_host() -> Option<MpvHost> {
    let cplugin = std::env::var_os("MPV_WGPU_CPLUGIN").map(PathBuf::from);
    let mpv = std::env::var_os("MPV_WGPU_MPV")
        .map(PathBuf::from)
        .or_else(|| {
            let path = std::env::var_os("PATH")?;
            std::env::split_paths(&path)
                .map(|dir| dir.join("mpv"))
                .find(|candidate| candidate.is_file())
        });
    match (mpv, cplugin) {
        (Some(mpv), Some(cplugin)) if cplugin.is_file() => Some(MpvHost { mpv, cplugin }),
        (None, _) => {
            eprintln!("SKIPPED: no mpv (set MPV_WGPU_MPV or put mpv on the search path)");
            None
        }
        (Some(_), _) => {
            eprintln!(
                "SKIPPED: no mpv-wgpu C plugin (build mpv-wgpu-cplugin, set MPV_WGPU_CPLUGIN)"
            );
            None
        }
    }
}

/// A device and queue, or `None` with a note when the machine has no adapter or no mpv: the
/// tests of a player that draws need both and say so rather than fail where there are none.
pub fn device() -> Option<(Device, Queue)> {
    mpv_host()?;
    match headless_device() {
        Ok(pair) => Some(pair),
        Err(error) => {
            eprintln!("SKIPPED: no graphics device ({error})");
            None
        }
    }
}

/// What the sink was told.
#[derive(Debug, Default)]
pub struct Shown {
    pub textures: usize,
    pub frames: usize,
    pub cleared: usize,
    pub last: Option<TextureView>,
}

#[derive(Clone, Default)]
pub struct Recording(pub Arc<Mutex<Shown>>);

impl Recording {
    /// How many times the sink was told of a texture or a frame.
    pub fn drawn(&self) -> usize {
        let seen = self.seen();
        seen.textures + seen.frames
    }

    pub fn seen(&self) -> std::sync::MutexGuard<'_, Shown> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl FrameSink for Recording {
    fn texture(&mut self, view: &TextureView) {
        let mut seen = self.seen();
        seen.textures += 1;
        seen.last = Some(view.clone());
    }

    fn frame(&mut self) {
        self.seen().frames += 1;
    }

    fn cleared(&mut self) {
        self.seen().cleared += 1;
    }
}

/// A driver on `file` with the sound off, and the channel mpv wakes it through.
pub struct Rig {
    pub driver: Driver,
    pub wakes: Receiver<()>,
    pub shown: Recording,
    pub events: Vec<MediaEvent>,
}

impl Rig {
    pub fn open(device: &Device, queue: &Queue, file: &FilePath) -> Rig {
        Rig::open_with(device, queue, &mpv_host().unwrap(), file)
    }

    /// A driver whose mpv is run as `host` says.
    pub fn open_with(device: &Device, queue: &Queue, host: &MpvHost, file: &FilePath) -> Rig {
        let (tx, wakes) = channel();
        let shown = Recording::default();
        let driver = Driver::open(
            device,
            queue,
            AudioDriver::Null,
            host,
            file,
            Box::new(shown.clone()),
            move || {
                let _ = tx.send(());
            },
        )
        .unwrap();
        Rig {
            driver,
            wakes,
            shown,
            events: Vec::new(),
        }
    }

    /// Send `command`, keeping what it caused.
    pub fn send(&mut self, command: MediaCommand) {
        let handled = self.driver.command(command);
        self.events.extend(handled.events);
    }

    /// Let the player work until `done` holds of the events heard so far.
    pub fn until(&mut self, what: &str, done: impl Fn(&[MediaEvent]) -> bool) {
        let started = Instant::now();
        while !done(&self.events) {
            assert!(
                started.elapsed() < TIMEOUT,
                "timed out waiting for {what}; heard {:#?}",
                self.events
            );
            let _ = self.wakes.recv_timeout(Duration::from_millis(100));
            let heard = self.driver.woken();
            self.events.extend(heard);
        }
    }

    pub fn has(&self, wanted: impl Fn(&MediaEvent) -> bool) -> bool {
        self.events.iter().any(wanted)
    }
}

/// The newest track list the player announced.
pub fn last_tracks(events: &[MediaEvent]) -> Option<&[MediaTrack]> {
    events.iter().rev().find_map(|event| match event {
        MediaEvent::Tracks(tracks) => Some(tracks.as_slice()),
        MediaEvent::Loaded { .. }
        | MediaEvent::Ended(_)
        | MediaEvent::Playback(_)
        | MediaEvent::SeekDone
        | MediaEvent::Buffering(_)
        | MediaEvent::Position(_)
        | MediaEvent::Length(_)
        | MediaEvent::Chapters(_)
        | MediaEvent::Volume(_)
        | MediaEvent::Speed(_)
        | MediaEvent::Picture(_)
        | MediaEvent::ShotSaved(_)
        | MediaEvent::ShotFailed { .. }
        | MediaEvent::Failed(_)
        | MediaEvent::Refused(_) => None,
    })
}

/// How many chapters the newest chapter list has.
pub fn last_chapters(events: &[MediaEvent]) -> Option<usize> {
    events.iter().rev().find_map(|event| match event {
        MediaEvent::Chapters(chapters) => Some(chapters.len()),
        MediaEvent::Loaded { .. }
        | MediaEvent::Ended(_)
        | MediaEvent::Playback(_)
        | MediaEvent::SeekDone
        | MediaEvent::Buffering(_)
        | MediaEvent::Position(_)
        | MediaEvent::Length(_)
        | MediaEvent::Tracks(_)
        | MediaEvent::Volume(_)
        | MediaEvent::Speed(_)
        | MediaEvent::Picture(_)
        | MediaEvent::ShotSaved(_)
        | MediaEvent::ShotFailed { .. }
        | MediaEvent::Failed(_)
        | MediaEvent::Refused(_) => None,
    })
}

/// How long the recording runs, as `Loaded` or, when that had none, `Length` said.
pub fn length_of(events: &[MediaEvent]) -> Option<anyview_core::MediaLength> {
    events.iter().find_map(|event| match event {
        MediaEvent::Loaded { length } => *length,
        MediaEvent::Length(length) => Some(*length),
        MediaEvent::Ended(_)
        | MediaEvent::Playback(_)
        | MediaEvent::SeekDone
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
        | MediaEvent::Refused(_) => None,
    })
}

/// Whether a position of `micros` microseconds was reported among `events`.
pub fn position_within(events: &[MediaEvent], micros: std::ops::RangeInclusive<u64>) -> bool {
    events
        .iter()
        .any(|event| matches!(event, MediaEvent::Position(at) if micros.contains(&at.0)))
}
