//! The pool, its runner for the views' work and the mailbox its endings come back through.

use crate::media::Exports;
use crate::runtime::{Lane, Mailbox, Pool, PoolSize, Runner, RuntimeError, UiWaker};
use anyview_core::work::{Backend, Stop, Ticketed};
use anyview_ui::{Work, WorkLane, Workers};
use std::sync::Arc;

use crate::runtime::JobOutcome;

/// How one piece of work ended, with the load it belonged to.
pub type Settled = Ticketed<JobOutcome<()>>;

/// The views' work as a back end: the job needs no document and no scratch, it is the whole
/// [`Work`], and it posts its own result.
struct WorkBackend;

impl Backend for WorkBackend {
    type Doc = ();
    type Worker = ();
    type Job = Work;
    type Done = ();

    fn run(_doc: &(), _worker: &mut (), job: Work, _stop: &Stop) {
        job.run();
    }
}

/// The pool's lane for work wanted this soon: what the person is looking at before what is read
/// ahead of them.
pub(super) fn lane_of(lane: WorkLane) -> Lane {
    match lane {
        WorkLane::Visible => Lane::Visible,
        WorkLane::Preload => Lane::Preload,
    }
}

/// The views' [`Workers`] on the runtime's pool.
struct PoolWorkers {
    runner: Runner<WorkBackend, Settled>,
}

impl Workers for PoolWorkers {
    fn submit(&self, work: Work) {
        let (ticket, lane) = (work.ticket(), lane_of(work.lane()));
        self.runner.submit(lane, ticket, Arc::new(()), work, None);
    }
}

/// The program's worker threads and everything the views reach them through. Dropping it joins
/// the workers.
pub struct Workforce {
    // Declared first so it drops first: the workers stop before the mailbox they post to goes.
    _pool: Pool,
    workers: Arc<dyn Workers>,
    exports: Arc<Exports>,
    mailbox: Mailbox<Settled>,
}

impl std::fmt::Debug for Workforce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workforce").finish_non_exhaustive()
    }
}

impl Workforce {
    /// `size` workers. `waker` is called when a job has ended and nothing is waiting to be told
    /// of an earlier one: the owner then calls [`Workforce::settled`].
    pub fn start(size: PoolSize, waker: impl UiWaker) -> Result<Workforce, RuntimeError> {
        let pool = Pool::new(size)?;
        let (mailbox, outbox) = Mailbox::new(waker);
        let exports = Arc::new(Exports::new(&pool, outbox.clone()));
        let runner = Runner::<WorkBackend, Settled>::new(&pool, outbox, || (), |ended| ended);
        Ok(Workforce {
            _pool: pool,
            workers: Arc::new(PoolWorkers { runner }),
            exports,
            mailbox,
        })
    }

    /// What the views submit their work to; clones share the pool.
    pub fn workers(&self) -> Arc<dyn Workers> {
        Arc::clone(&self.workers)
    }

    /// The pool's runner for media exports; clones share the pool.
    pub fn exports(&self) -> Arc<Exports> {
        Arc::clone(&self.exports)
    }

    /// How the jobs that ended since the last call ended, oldest first.
    pub fn settled(&self) -> Vec<Settled> {
        self.mailbox.drain()
    }
}
