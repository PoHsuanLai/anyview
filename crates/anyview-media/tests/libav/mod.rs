//! Shared by the media tests: the fixtures, a tagged recording made in memory, and a way to run an
//! export the way the pool does.

#![allow(clippy::unwrap_used, dead_code)]

use anyview_core::work::{Backend, Stop};
use anyview_core::{ExportJob, FilePath, MediaLength, StreamPick, TimeRange};
use anyview_media::{
    ExportBackend, ExportProgress, ExportReport, ExportRequest, MediaError, MediaProbe, probe,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn file(path: &Path) -> FilePath {
    FilePath::new(path).unwrap()
}

pub fn probed(path: &Path) -> MediaProbe {
    probe(path).unwrap()
}

/// The seconds a probe says a recording runs.
pub fn seconds(probe: &MediaProbe) -> f64 {
    let MediaLength(time) = probe.length.expect("the recording has a length");
    time.0 as f64 / 1_000_000.0
}

/// Half a second of 8 kHz mono 16-bit sound as a WAV file that carries a title, an artist and an
/// album in its INFO list.
pub fn tagged_wav() -> Vec<u8> {
    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend((body.len() as u32).to_le_bytes());
        bytes.extend(body);
        if body.len() % 2 == 1 {
            bytes.push(0);
        }
        bytes
    }
    let text = |value: &str| {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        bytes
    };
    let mut format = Vec::new();
    format.extend(1_u16.to_le_bytes());
    format.extend(1_u16.to_le_bytes());
    format.extend(8000_u32.to_le_bytes());
    format.extend(16_000_u32.to_le_bytes());
    format.extend(2_u16.to_le_bytes());
    format.extend(16_u16.to_le_bytes());
    let samples: Vec<u8> = (0..4000)
        .map(|n| (f64::from(n) * 0.2).sin() * 8000.0)
        .flat_map(|value| (value as i16).to_le_bytes())
        .collect();
    let mut info = b"INFO".to_vec();
    info.extend(chunk(b"INAM", &text("Song")));
    info.extend(chunk(b"IART", &text("Band")));
    info.extend(chunk(b"IPRD", &text("Record")));
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &format));
    body.extend(chunk(b"LIST", &info));
    body.extend(chunk(b"data", &samples));
    chunk(b"RIFF", &body)
}

/// Run `job` into `to` as a pool worker would, recording every progress report.
pub fn run(
    job: ExportJob,
    to: &Path,
    stop: &Stop,
) -> (Result<ExportReport, MediaError>, Vec<ExportProgress>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let request = ExportRequest {
        job,
        to: file(to),
        progress: Arc::new(move |progress| sink.lock().unwrap().push(progress)),
    };
    let done = ExportBackend::run(&(), &mut (), request, stop);
    let reports = seen.lock().unwrap().clone();
    (done, reports)
}

pub fn transcode(
    source: &Path,
    range: TimeRange,
    streams: StreamPick,
    audio: anyview_core::AudioTarget,
) -> ExportJob {
    ExportJob::Transcode {
        source: file(source),
        range,
        streams,
        audio,
    }
}
