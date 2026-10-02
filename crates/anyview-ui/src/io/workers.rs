//! How work leaves the window and its result comes back. The window never runs a [`Job`] itself
//! and the library never spawns a thread: it hands each [`Work`] to the binary's [`Workers`]
//! (its one bounded pool), a worker calls [`Work::run`], and the [`Done`] it makes is posted to
//! the window's mailbox through the [`Reply`] the work carries. Results are machine inputs with
//! the load's ticket, so one that arrives after the person left the file is a listed no-op.

use super::job::{Done, Job};
use crate::sheet::ExportDraft;
use crate::{Presentation, TypedText};
use anyview_core::{FileAction, Resume};
use anyview_text::Highlighter;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::sync::{Arc, Mutex, PoisonError};

/// The binary's pool. Implemented over its threads; a test implements it to run work inline.
pub trait Workers: Send + Sync + 'static {
    /// Run `work` on a worker thread, soon. It blocks that thread and nothing else.
    fn submit(&self, work: Work);
}

/// One job and the way back from it.
pub struct Work {
    job: Job,
    reply: Reply,
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Work").field("job", &self.job).finish()
    }
}

impl Work {
    /// Do the job, blocking, and post what it made to the window that asked.
    pub fn run(self) {
        let Work { job, reply } = self;
        reply.post(job.run());
    }
}

/// Where a worker leaves the result of a job: the window's mailbox. Cloneable and sendable.
#[derive(Debug, Clone)]
pub struct Reply(UnboundedSender<Done>);

impl Reply {
    /// Leave `done` in the mailbox. A window that closed has no mailbox, and the result is
    /// dropped: nobody is waiting for it.
    pub fn post(&self, done: Done) {
        let _closed = self.0.unbounded_send(done);
    }
}

/// What the window asks of the binary that hosts it: the things the viewer decides to do but
/// cannot do itself (they touch the platform, the file system or the window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRequest {
    /// Carry out a file action on the open file (reveal it, copy it, open it with, print it…).
    Run(FileAction),
    /// Choose another file to open.
    PickFile,
    /// Close this window.
    CloseWindow,
    /// Write this export of the open file.
    Export(ExportDraft),
    /// Move the open file to the trash.
    Trash,
    /// Rename the open file.
    Rename(TypedText),
    /// Keep where the person is in the open file, for next time.
    Remember(Resume),
    /// Show the window this way.
    Present(Presentation),
}

/// What one viewer window is wired to: the workers, the way back from them, and the binary's
/// requests. The binary makes one per window and gives it to the window's root as a context
/// (`ds_blitz::AppConfig::with_context`).
#[derive(Clone)]
pub struct Edge {
    workers: Arc<dyn Workers>,
    reply: Reply,
    mailbox: Arc<Mutex<Option<UnboundedReceiver<Done>>>>,
    requests: Arc<dyn Fn(HostRequest) + Send + Sync>,
    highlighter: Arc<Highlighter>,
}

impl std::fmt::Debug for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Edge").finish_non_exhaustive()
    }
}

impl Edge {
    /// An edge over `workers`, handing the window's requests to `requests`.
    pub fn new(
        workers: Arc<dyn Workers>,
        requests: impl Fn(HostRequest) + Send + Sync + 'static,
    ) -> Edge {
        let (sender, receiver) = unbounded();
        Edge {
            workers,
            reply: Reply(sender),
            mailbox: Arc::new(Mutex::new(Some(receiver))),
            requests: Arc::new(requests),
            highlighter: Arc::new(Highlighter::new()),
        }
    }

    /// Ask a worker to do `job`; its result arrives in the mailbox.
    pub(crate) fn submit(&self, job: Job) {
        self.workers.submit(Work {
            job,
            reply: self.reply.clone(),
        });
    }

    /// The mailbox, once: the window's UI task takes it and awaits the results.
    pub(crate) fn take_mailbox(&self) -> Option<UnboundedReceiver<Done>> {
        self.mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// The one highlighter every file of the window shares.
    pub(crate) fn highlighter(&self) -> Arc<Highlighter> {
        Arc::clone(&self.highlighter)
    }

    /// Hand a request to the binary.
    pub(crate) fn request(&self, request: HostRequest) {
        (self.requests)(request);
    }
}
