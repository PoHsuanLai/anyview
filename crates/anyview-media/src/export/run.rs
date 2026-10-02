//! Running one export: pick the job's way of writing, write beside the target under a temporary
//! name and put the file in place only when it is whole.

use super::convert::convert;
use super::copy::copy;
use super::encoders::Encoders;
use super::gate::Reporter;
use super::request::{ExportReport, ExportRequest};
use crate::MediaError;
use crate::libav::{file_path, start};
use crate::probe::probe_within;
use anyview_core::work::{Backend, Stop, StopState};
use anyview_core::{AudioTarget, ExportJob, MediaLength, MediaTime, StreamPick, TimeRange};
use std::path::{Path, PathBuf};

/// The media export as a pool back end: a job needs no shared document and no scratch, it opens
/// its own libav contexts on the worker that runs it and never moves them between threads.
#[derive(Debug, Clone, Copy)]
pub struct ExportBackend;

impl Backend for ExportBackend {
    type Doc = ();
    type Worker = ();
    type Job = ExportRequest;
    type Done = Result<ExportReport, MediaError>;

    /// Writes the file, or leaves nothing: a raised `stop` ends it between packets, removes what
    /// was written and answers [`MediaError::Stopped`].
    fn run(_: &(), _: &mut (), job: ExportRequest, stop: &Stop) -> Self::Done {
        export(job, stop)
    }
}

/// The part of the recording an export writes: from the range's start to its end or the
/// recording's.
fn written_length(range: TimeRange, whole: Option<MediaLength>) -> Option<MediaLength> {
    let end = range.end().or(whole.map(|length| length.0))?;
    Some(MediaLength(MediaTime(
        end.0.saturating_sub(range.start().0),
    )))
}

fn export(request: ExportRequest, stop: &Stop) -> Result<ExportReport, MediaError> {
    let ExportJob::Transcode {
        source,
        range,
        streams,
        audio,
    } = &request.job
    else {
        return Err(MediaError::NotMedia);
    };
    if stop.stopped() == StopState::Stopped {
        return Err(MediaError::Stopped);
    }
    start()?;
    let encoders = Encoders::detect()?;
    encoders.supports(*audio)?;
    let probe = probe_within(source.as_path(), 0)?;
    let mut reporter = Reporter::new(
        request.progress.clone(),
        written_length(*range, probe.length),
    );
    let temp = temporary(request.to.as_path());
    let written = match (streams, audio) {
        (StreamPick::Everything, _) | (StreamPick::AudioOnly, AudioTarget::Copy) => copy(
            source.as_path(),
            &temp,
            *range,
            *streams,
            &mut reporter,
            stop,
        ),
        (
            StreamPick::AudioOnly,
            AudioTarget::M4a(_)
            | AudioTarget::Mp3(_)
            | AudioTarget::Flac
            | AudioTarget::Wav
            | AudioTarget::Opus(_),
        ) => convert(
            source.as_path(),
            &temp,
            *range,
            *audio,
            &encoders,
            &mut reporter,
            stop,
        ),
    };
    match written {
        Ok(length) => {
            std::fs::rename(&temp, request.to.as_path()).map_err(|error| {
                let _ = std::fs::remove_file(&temp);
                MediaError::Io {
                    op: "write",
                    path: request.to.as_path().to_path_buf(),
                    kind: error.kind(),
                }
            })?;
            Ok(ExportReport {
                path: file_path(request.to.as_path())?,
                length,
            })
        }
        Err(error) => {
            // A failed or stopped export leaves nothing: the half-written file is the only
            // thing there is to remove, and it may not exist if libav failed before creating it.
            let _ = std::fs::remove_file(&temp);
            Err(error)
        }
    }
}

/// The name an export is written under until it is whole: beside the target, keeping its
/// extension so libav picks the container from it.
fn temporary(to: &Path) -> PathBuf {
    let name = to
        .file_name()
        .map_or_else(|| "export".into(), std::ffi::OsStr::to_os_string);
    let mut hidden = std::ffi::OsString::from(".part-");
    hidden.push(name);
    to.with_file_name(hidden)
}
