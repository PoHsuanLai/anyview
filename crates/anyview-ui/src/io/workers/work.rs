//! A job handed to the binary's pool: the `Work` a worker runs, the `Workers` that run it, and the
//! `Reply` a result goes back through.

use super::super::job::{Done, Job, Replaces, WorkLane};
use super::super::seams::{FileLocks, ResumeSource, VersionSource};
use crate::Ticket;
use anyview_core::work::{Stop, StopState};
use futures_channel::mpsc::UnboundedSender;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

/// The binary's pool. Implemented over its threads; a test implements it to run work inline.
pub trait Workers: Send + Sync + 'static {
    /// Run `work` on a worker thread, soon. It blocks that thread and nothing else.
    fn submit(&self, work: Work);
}

/// What a piece of work does, for a pool that logs, counts or tests it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkKind {
    /// Sniff a file.
    Probe,
    /// Make a file's first frame.
    Peek,
    /// Open a file.
    Open,
    /// Read a window of lines.
    Lines,
    /// Search an open file for a phrase: a text, a table's cells, a tree's keys and values.
    Search,
    /// Sort a table sheet by a column.
    Sort,
    /// Read a text whole, to edit it.
    EditRead,
    /// Build a table's file with its edits made, to write it.
    DataBytes,
    /// Open a neighbour ahead of time.
    Preload,
    /// Read a file's stamp.
    Stat,
    /// List a folder.
    Folder,
    /// Draw tiles, a thumbnail or a search of an open PDF.
    Pdf,
    /// List the versions kept of a file.
    Versions,
    /// Write a view or a place to the host's store.
    Keep,
}

/// One job and the way back from it.
pub struct Work {
    pub(super) job: Job,
    pub(super) reply: Reply,
    pub(super) resume: Arc<dyn ResumeSource>,
    pub(super) locks: Arc<dyn FileLocks>,
    pub(super) versions: Arc<dyn VersionSource>,
    pub(super) stop: Stop,
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Work").field("job", &self.job).finish()
    }
}

/// What a panic said: its message when it was a string, which `panic!` with text always is.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message")
}

impl Work {
    /// The queue this belongs on: the job decides.
    #[must_use]
    pub fn lane(&self) -> WorkLane {
        self.job.lane()
    }

    /// The load this work belongs to: the pool tags its own delivery with it. Work that belongs
    /// to no load (a neighbour opened ahead, a stamp, a folder) says `Ticket::default()`.
    #[must_use]
    pub fn ticket(&self) -> Ticket {
        self.job.ticket()
    }

    /// Do the job, blocking, and post what it made to the window that asked. Work for a load the
    /// person has since left is not started, and is told to end early when it is running; a
    /// search that a newer query replaced answers nothing. A job that panics (a decoder fed a
    /// hostile file) is answered as a failure, so the window never waits on a result that cannot
    /// come.
    pub fn run(self) {
        let kind = self.kind();
        let Work {
            job,
            reply,
            resume,
            locks,
            versions,
            stop,
        } = self;
        if stop.stopped() == StopState::Stopped {
            return;
        }
        let failure = job.crashed();
        let replaceable = job.replaces() != Replaces::Nothing;
        let ran = catch_unwind(AssertUnwindSafe(|| {
            job.run(resume.as_ref(), locks.as_ref(), versions.as_ref(), &stop)
        }));
        // A search that was replaced while it read has only some of the hits, and a window of
        // lines that was replaced has been scrolled past: nobody waits on either.
        if replaceable && stop.stopped() == StopState::Stopped {
            return;
        }
        let done = ran.unwrap_or_else(|payload| {
            log::error!(
                "a {kind:?} job panicked: {}",
                panic_message(payload.as_ref())
            );
            failure
        });
        reply.post(done);
    }

    /// Whether the work has been told to end, because a newer load or a newer search replaced
    /// it: a pool that queues work can drop it unstarted.
    #[must_use]
    pub fn replaced(&self) -> bool {
        self.stop.stopped() == StopState::Stopped
    }

    /// What the work does.
    #[must_use]
    pub fn kind(&self) -> WorkKind {
        match &self.job {
            Job::Probe { .. } => WorkKind::Probe,
            Job::Peek { .. } => WorkKind::Peek,
            Job::Open { .. } => WorkKind::Open,
            Job::Lines { .. } => WorkKind::Lines,
            Job::Search { .. } | Job::SearchCells { .. } | Job::SearchNodes { .. } => {
                WorkKind::Search
            }
            Job::Sort { .. } => WorkKind::Sort,
            Job::EditRead { .. } => WorkKind::EditRead,
            Job::DataBytes { .. } => WorkKind::DataBytes,
            Job::Preload { .. } => WorkKind::Preload,
            Job::Stat { .. } => WorkKind::Stat,
            Job::Folder { .. } => WorkKind::Folder,
            Job::Pdf(_) => WorkKind::Pdf,
            Job::Versions { .. } => WorkKind::Versions,
            Job::Keep { .. } => WorkKind::Keep,
        }
    }
}

/// Where a worker leaves the result of a job: the window's mailbox. Cloneable and sendable.
#[derive(Debug, Clone)]
pub struct Reply(pub(super) UnboundedSender<Done>);

impl Reply {
    /// Leave `done` in the mailbox. A window that closed has no mailbox, and the result is
    /// dropped: nobody is waiting for it.
    pub fn post(&self, done: Done) {
        let _closed = self.0.unbounded_send(done);
    }
}
