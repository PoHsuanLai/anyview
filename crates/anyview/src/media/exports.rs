//! Media exports on the pool: a transcode is blocking libav work for one worker, with progress
//! and a `Stop`; the frame comes from the player that shows it.

use crate::runtime::{JobHandle, JobOutcome, Lane, Pool, Runner};
use crate::seam::Settled;
use anyview_core::work::{Ticket, Ticketed};
use anyview_media::{ExportBackend, ExportReport, ExportRequest, MediaError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::oneshot;

/// How an export ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportEnd {
    /// It wrote its file.
    Written(ExportReport),
    /// libav refused, a codec is missing, or it was stopped.
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
    runner: Runner<ExportBackend, Settled>,
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

    /// Queue `request`.
    pub fn submit(&self, request: ExportRequest) -> ExportHandle {
        let ticket = Ticket(self.tickets.fetch_add(1, Ordering::Relaxed));
        let (tell, ended) = oneshot::channel();
        self.waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(ticket, tell);
        let job = self
            .runner
            .submit(Lane::Visible, ticket, Arc::new(()), request, None);
        ExportHandle { job, ended }
    }
}
