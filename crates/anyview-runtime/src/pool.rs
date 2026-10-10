//! The bounded pool: worker threads that take jobs from two lanes, visible work before preload.

use super::RuntimeError;
use anyview_core::work::Backend;
use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

/// Which queue a job waits in. A worker takes from `Visible` whenever it has anything, so what
/// the person is looking at is never behind a preload; within a lane jobs run in the order they
/// were submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    /// Work the shown file needs now.
    Visible,
    /// Work done ahead of need (the next page, the next file).
    Preload,
}

/// How many worker threads the pool runs: at least one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PoolSize(NonZeroUsize);

impl PoolSize {
    /// One less than the cores, leaving one for the UI thread, and never fewer than one.
    pub fn from_cores(cores: NonZeroUsize) -> Self {
        PoolSize(NonZeroUsize::new(cores.get() - 1).unwrap_or(NonZeroUsize::MIN))
    }

    /// Exactly `threads` workers.
    pub fn exactly(threads: NonZeroUsize) -> Self {
        PoolSize(threads)
    }

    /// The number of workers.
    pub fn get(self) -> usize {
        self.0.get()
    }
}

/// Each worker's scratch, one per backend that has run on it. Owned by the worker thread.
#[derive(Default)]
pub(super) struct Scratch {
    workers: HashMap<TypeId, Box<dyn Any>>,
}

impl Scratch {
    /// This thread's `B::Worker`, made with `make` the first time.
    pub(super) fn worker<B: Backend>(&mut self, make: &dyn Fn() -> B::Worker) -> &mut B::Worker
    where
        B::Worker: 'static,
    {
        let slot = self
            .workers
            .entry(TypeId::of::<B>())
            .or_insert_with(|| Box::new(make()));
        // The slot under `TypeId::of::<B>()` only ever holds a `B::Worker`.
        match slot.downcast_mut::<B::Worker>() {
            Some(worker) => worker,
            None => unreachable!("scratch slot holds the worker type of its backend"),
        }
    }

    /// Forgets `B`'s scratch, so the next job makes a fresh one (after a panic left it unknown).
    pub(super) fn discard<B: Backend>(&mut self) {
        self.workers.remove(&TypeId::of::<B>());
    }
}

pub(super) type Task = Box<dyn FnOnce(&mut Scratch) + Send>;

#[derive(Default)]
struct Queues {
    visible: VecDeque<Task>,
    preload: VecDeque<Task>,
    closing: bool,
}

struct Shared {
    queues: Mutex<Queues>,
    ready: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queues> {
        self.queues.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The way to put tasks on a pool; clones share it. Submitting to a pool that has shut down drops
/// the task.
#[derive(Clone)]
pub(super) struct Spawner {
    shared: Arc<Shared>,
}

impl Spawner {
    pub(super) fn spawn(&self, lane: Lane, task: Task) {
        let mut queues = self.shared.lock();
        if queues.closing {
            return;
        }
        match lane {
            Lane::Visible => queues.visible.push_back(task),
            Lane::Preload => queues.preload.push_back(task),
        }
        drop(queues);
        self.shared.ready.notify_one();
    }
}

/// A bounded set of worker threads. Dropping it stops the workers after their current job,
/// discards what is still queued, and joins them.
pub struct Pool {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("threads", &self.threads.len())
            .finish()
    }
}

impl Pool {
    /// Starts `size` workers named `anyview-worker-N`.
    pub fn new(size: PoolSize) -> Result<Pool, RuntimeError> {
        let shared = Arc::new(Shared {
            queues: Mutex::new(Queues::default()),
            ready: Condvar::new(),
        });
        // Built before the threads so a failed spawn drops it, which stops the ones that started.
        let mut pool = Pool {
            shared: Arc::clone(&shared),
            threads: Vec::new(),
        };
        for index in 0..size.get() {
            let name = format!("anyview-worker-{index}");
            let worker_shared = Arc::clone(&shared);
            let thread = std::thread::Builder::new()
                .name(name.clone())
                .spawn(move || work(&worker_shared))
                .map_err(|source| RuntimeError::Spawn { name, source })?;
            pool.threads.push(thread);
        }
        Ok(pool)
    }

    pub(super) fn spawner(&self) -> Spawner {
        Spawner {
            shared: Arc::clone(&self.shared),
        }
    }

    /// How many workers the pool runs.
    pub fn size(&self) -> usize {
        self.threads.len()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        {
            let mut queues = self.shared.lock();
            queues.closing = true;
            queues.visible.clear();
            queues.preload.clear();
        }
        self.shared.ready.notify_all();
        for thread in self.threads.drain(..) {
            // A worker never panics out of its loop (jobs are caught), so there is nothing to act on.
            let _ = thread.join();
        }
    }
}

fn next_task(shared: &Shared) -> Option<Task> {
    let mut queues = shared.lock();
    loop {
        if queues.closing {
            return None;
        }
        if let Some(task) = queues.visible.pop_front() {
            return Some(task);
        }
        if let Some(task) = queues.preload.pop_front() {
            return Some(task);
        }
        queues = shared
            .ready
            .wait(queues)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

fn work(shared: &Shared) {
    let mut scratch = Scratch::default();
    while let Some(task) = next_task(shared) {
        // A task catches its job's panic; this guards the delivery around it, so no panic ends a worker.
        let _ = catch_unwind(AssertUnwindSafe(|| task(&mut scratch)));
    }
}
