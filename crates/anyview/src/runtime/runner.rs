//! Running one back end's jobs on the pool and delivering what they produce.

use super::mailbox::Outbox;
use super::pool::{Lane, Scratch, Spawner};
use anyview_core::work::{Backend, Stop, StopState, Ticket, Ticketed};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Instant;

/// A job panicked. The pool carries on and the worker's scratch for that back end is rebuilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPanic {
    /// The panic's message, when it had one.
    pub message: String,
}

/// What became of a submitted job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome<D> {
    /// The back end ran and returned (possibly early, if it saw its stop).
    Done(D),
    /// The stop was raised or the deadline passed before a worker picked the job up: it never ran.
    Skipped,
    /// The job panicked.
    Panicked(JobPanic),
}

/// Turns a finished job into the input the machines take.
type IntoInput<B, T> = dyn Fn(Ticketed<JobOutcome<<B as Backend>::Done>>) -> T + Send + Sync;

/// A submitted job: its ticket, and its `Stop` to raise if the result is no longer wanted.
#[derive(Debug, Clone)]
pub struct JobHandle {
    ticket: Ticket,
    stop: Stop,
}

impl JobHandle {
    /// The load this job belongs to.
    pub fn ticket(&self) -> Ticket {
        self.ticket
    }

    /// The job's stop; clones share it.
    pub fn stop(&self) -> &Stop {
        &self.stop
    }

    /// Asks the job to end: a job still queued is skipped, one running sees it in its `Stop`.
    pub fn cancel(&self) {
        self.stop.request();
    }
}

/// Runs the jobs of one back end `B` on a pool and posts each result, as the input `T` the
/// machines take, to the UI thread's mailbox. Stale tickets are not filtered here: the machine
/// that issued the ticket knows which is current.
pub struct Runner<B: Backend, T> {
    spawner: Spawner,
    make_worker: Arc<dyn Fn() -> B::Worker + Send + Sync>,
    outbox: Outbox<T>,
    into_input: Arc<IntoInput<B, T>>,
}

impl<B: Backend, T> std::fmt::Debug for Runner<B, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner").finish_non_exhaustive()
    }
}

impl<B, T> Runner<B, T>
where
    B: Backend,
    B::Worker: 'static,
    B::Doc: 'static,
    B::Job: 'static,
    B::Done: 'static,
    T: Send + 'static,
{
    /// A runner on `pool`. `make_worker` builds a worker's scratch the first time a thread runs
    /// one of this back end's jobs; `into_input` turns a ticketed outcome into the machine input.
    pub fn new(
        pool: &super::Pool,
        outbox: Outbox<T>,
        make_worker: impl Fn() -> B::Worker + Send + Sync + 'static,
        into_input: impl Fn(Ticketed<JobOutcome<B::Done>>) -> T + Send + Sync + 'static,
    ) -> Self {
        Self {
            spawner: pool.spawner(),
            make_worker: Arc::new(make_worker),
            outbox,
            into_input: Arc::new(into_input),
        }
    }

    /// Queues `job` on `lane` for the load `ticket` names; `deadline` ends it at that instant.
    /// The result is posted when the job finishes, is skipped or panics.
    pub fn submit(
        &self,
        lane: Lane,
        ticket: Ticket,
        doc: Arc<B::Doc>,
        job: B::Job,
        deadline: Option<Instant>,
    ) -> JobHandle {
        let stop = match deadline {
            Some(deadline) => Stop::with_deadline(deadline),
            None => Stop::new(),
        };
        let handle = JobHandle {
            ticket,
            stop: stop.clone(),
        };
        let make_worker = Arc::clone(&self.make_worker);
        let into_input = Arc::clone(&self.into_input);
        let outbox = self.outbox.clone();
        self.spawner.spawn(
            lane,
            Box::new(move |scratch: &mut Scratch| {
                let outcome = execute::<B>(scratch, &*make_worker, &doc, job, &stop);
                outbox.send(into_input(Ticketed::new(ticket, outcome)));
            }),
        );
        handle
    }
}

fn execute<B>(
    scratch: &mut Scratch,
    make_worker: &dyn Fn() -> B::Worker,
    doc: &B::Doc,
    job: B::Job,
    stop: &Stop,
) -> JobOutcome<B::Done>
where
    B: Backend,
    B::Worker: 'static,
{
    match stop.stopped_at(Instant::now()) {
        StopState::Stopped => return JobOutcome::Skipped,
        StopState::Running => {}
    }
    let ran = catch_unwind(AssertUnwindSafe(|| {
        B::run(doc, scratch.worker::<B>(make_worker), job, stop)
    }));
    match ran {
        Ok(done) => JobOutcome::Done(done),
        Err(payload) => {
            scratch.discard::<B>();
            JobOutcome::Panicked(JobPanic {
                message: panic_message(payload.as_ref()),
            })
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        String::new()
    }
}
