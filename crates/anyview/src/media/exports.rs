//! Media exports on the pool: a transcode is a request to the FFmpeg plugin that one worker waits
//! on, with progress and a `Stop` that cancels it; the frame comes from the player that shows it.

use crate::runtime::{JobHandle, JobOutcome, Lane, Pool, Runner};
use crate::seam::Settled;
use anyview_core::work::{Backend, Stop, Ticket, Ticketed};
use anyview_core::{MediaLength, MediaTime};
use anyview_media::{ExportProgress, ExportReport, ExportRequest, MediaError, ask_of};
use anyview_media_host::ExportTool;
use anyview_platform::PlatformError;
use anyview_plugin_protocol::{ExportRequest as PluginRequest, MicroRange};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::oneshot;

/// How an export ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportEnd {
    /// It wrote its file.
    Written(ExportReport),
    /// The plugin refused, a codec is missing, or it was stopped.
    Failed(MediaError),
    /// The job panicked, with its message.
    Panicked(String),
    /// It was stopped before a worker took it up.
    Skipped,
}

/// A submitted export: stop it, or wait for how it ended.
#[derive(Debug)]
pub struct ExportHandle {
    job: JobHandle,
    ended: oneshot::Receiver<ExportEnd>,
}

impl ExportHandle {
    /// Ask the export to stop: it removes what it wrote and ends with `MediaError::Stopped`.
    pub fn stop(&self) {
        self.job.cancel();
    }

    /// Wait for the export to end.
    pub async fn ended(self) -> ExportEnd {
        self.ended.await.unwrap_or(ExportEnd::Skipped)
    }
}

type Waiting = Mutex<HashMap<Ticket, oneshot::Sender<ExportEnd>>>;

/// The pool's runner for media exports.
pub struct Exports {
    runner: Runner<PluginExport, Settled>,
    waiting: Arc<Waiting>,
    tickets: AtomicU64,
}

impl std::fmt::Debug for Exports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exports").finish_non_exhaustive()
    }
}

impl Exports {
    /// Export jobs on `pool`; how each ended is also posted to `outbox`, for the program's report
    /// of a job that panicked.
    pub(crate) fn new(pool: &Pool, outbox: crate::runtime::Outbox<Settled>) -> Exports {
        let waiting: Arc<Waiting> = Arc::default();
        let tell = Arc::clone(&waiting);
        let runner = Runner::new(
            pool,
            outbox,
            || (),
            move |ended: Ticketed<JobOutcome<Result<ExportReport, MediaError>>>| {
                let ticket = ended.ticket;
                let (end, settled) = match ended.value {
                    JobOutcome::Done(Ok(report)) => {
                        (ExportEnd::Written(report), JobOutcome::Done(()))
                    }
                    JobOutcome::Done(Err(error)) => {
                        (ExportEnd::Failed(error), JobOutcome::Done(()))
                    }
                    JobOutcome::Panicked(panic) => (
                        ExportEnd::Panicked(panic.message.clone()),
                        JobOutcome::Panicked(panic),
                    ),
                    JobOutcome::Skipped => (ExportEnd::Skipped, JobOutcome::Skipped),
                };
                let waiter = tell
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&ticket);
                if let Some(waiter) = waiter {
                    let _gone = waiter.send(end);
                }
                Ticketed::new(ticket, settled)
            },
        );
        Exports {
            runner,
            waiting,
            tickets: AtomicU64::new(1),
        }
    }

    /// Queue `request`, to be written by the plugin `tool` names.
    pub fn submit(&self, tool: Arc<ExportTool>, request: ExportRequest) -> ExportHandle {
        let ticket = Ticket(self.tickets.fetch_add(1, Ordering::Relaxed));
        let (tell, ended) = oneshot::channel();
        self.waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(ticket, tell);
        let job = self
            .runner
            .submit(Lane::Visible, ticket, tool, request, None);
        ExportHandle { job, ended }
    }
}

/// The media export as a pool back end: the job is a request to a plugin, and the worker waits
/// for its answer, telling the request's sink how far it is and cancelling it when stopped.
#[derive(Debug, Clone, Copy)]
pub struct PluginExport;

impl Backend for PluginExport {
    type Doc = ExportTool;
    type Worker = ();
    type Job = ExportRequest;
    type Done = Result<ExportReport, MediaError>;

    fn run(tool: &ExportTool, _: &mut (), job: ExportRequest, stop: &Stop) -> Self::Done {
        let ask = ask_of(&job.job)?;
        let anyview_core::ExportJob::Transcode { source, .. } = &job.job else {
            return Err(MediaError::NotMedia);
        };
        let request = PluginRequest {
            input: source.as_path().to_path_buf(),
            output: job.to.as_path().to_path_buf(),
            target: ask.target.to_owned(),
            range: (ask.from > MediaTime::default() || ask.to.is_some()).then(|| MicroRange {
                start: ask.from.0,
                end: ask.to.map(|at| at.0),
            }),
            stream: None,
            bitrate: ask.bitrate.map(|rate| rate.kbps().saturating_mul(1000)),
        };
        let progress = job.progress.clone();
        let written = tool
            .runner
            .export(&tool.plugin, &request, stop, |said| {
                (progress)(ExportProgress {
                    done: MediaTime(said.done),
                    of: (said.total > 0).then_some(MediaLength(MediaTime(said.total))),
                });
            })
            .map_err(failure)?;
        let path = match written.output {
            Some(other) => anyview_core::FilePath::new(other).unwrap_or(job.to),
            None => job.to,
        };
        Ok(ExportReport { path })
    }
}

/// A plugin's failure as the crate that plans exports reports it: a stopped export is stopped,
/// anything else carries the plugin's own words.
fn failure(error: PlatformError) -> MediaError {
    if matches!(error, PlatformError::PluginCancelled { .. }) {
        MediaError::Stopped
    } else {
        MediaError::Export(error.to_string())
    }
}
