//! How work leaves the window and its result comes back. The window never runs a [`Job`] itself
//! and the library never spawns a thread: it hands each [`Work`] to the binary's [`Workers`]
//! (its one bounded pool), a worker calls [`Work::run`], and the [`Done`] it makes is posted to
//! the window's mailbox through the [`Reply`] the work carries. Results are machine inputs with
//! the load's ticket, so one that arrives after the person left the file is a listed no-op.

use super::helpers::{HelperSource, NoHelpers};
use super::job::{Done, Job, OpenLink, Probed, WorkLane};
use super::media::{MediaHost, MediaPort, NoPlayer};
use super::notice::Notice;
use super::seams::{
    FileCards, FileLocks, FirstFrameSource, Forgetful, ImagePlugins, NoCards, NoImagePlugins,
    NoLocks, NoPictures, NoVersions, ResumeSource, VersionSource,
};
use crate::edits::{EditRequest, Rewind};
use crate::sheet::{ExportDraft, VersionKey};
use crate::{Presentation, Ticket, TypedText};
use anyview_core::work::{Stop, StopState};
use anyview_core::{FileAction, FilePath, Helper, PixelSize, Resume};
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};

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
    /// Search a text for a phrase.
    Search,
    /// Unpack one section of an open book.
    Section,
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
}

/// One job and the way back from it.
pub struct Work {
    job: Job,
    reply: Reply,
    resume: Arc<dyn ResumeSource>,
    locks: Arc<dyn FileLocks>,
    versions: Arc<dyn VersionSource>,
    stop: Stop,
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Work").field("job", &self.job).finish()
    }
}

impl Work {
    /// The queue this belongs on: the job decides.
    pub fn lane(&self) -> WorkLane {
        self.job.lane()
    }

    /// The load this work belongs to: the pool tags its own delivery with it. Work that belongs
    /// to no load (a neighbour opened ahead, a stamp, a folder) says `Ticket::default()`.
    pub fn ticket(&self) -> Ticket {
        self.job.ticket()
    }

    /// Do the job, blocking, and post what it made to the window that asked. Work for a load the
    /// person has since left is not started, and is told to end early when it is running; a job
    /// that panics (a decoder fed a hostile file) is answered as a failure, so the window never
    /// waits on a result that cannot come.
    pub fn run(self) {
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
        let ran = catch_unwind(AssertUnwindSafe(|| {
            job.run(resume.as_ref(), locks.as_ref(), versions.as_ref(), &stop)
        }));
        reply.post(ran.unwrap_or(failure));
    }

    /// What the work does.
    pub fn kind(&self) -> WorkKind {
        match &self.job {
            Job::Probe { .. } => WorkKind::Probe,
            Job::Peek { .. } => WorkKind::Peek,
            Job::Open { .. } => WorkKind::Open,
            Job::Lines { .. } => WorkKind::Lines,
            Job::Search { .. } => WorkKind::Search,
            Job::Section { .. } => WorkKind::Section,
            Job::Preload { .. } => WorkKind::Preload,
            Job::Stat { .. } => WorkKind::Stat,
            Job::Folder { .. } => WorkKind::Folder,
            Job::Pdf(_) => WorkKind::Pdf,
            Job::Versions { .. } => WorkKind::Versions,
        }
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
    /// The window now shows this file: the host records it as viewed, and it is the file the
    /// requests below that name none refer to.
    Opened(Probed),
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
    /// Save the open file in place with this change; the host keeps the original first.
    Edit(EditRequest),
    /// Take back the last edit of the open file, or do it again.
    Rewind(Rewind),
    /// Put this kept version back as the open file.
    RevertTo(VersionKey),
    /// Write a copy of the open file under this name or at this path.
    SaveCopy(TypedText),
    /// Keep where the person is in the open file, for next time. Sent whenever a gesture settles,
    /// so the host may coalesce them.
    Remember(Resume),
    /// Watch this file for changes on disk and tell the window (`Edge::changed`); this replaces
    /// the window's earlier watch, and watching the file already watched is no change.
    Watch(FilePath),
    /// Stop watching: the window has no file open.
    Unwatch,
    /// Show the window this way.
    Present(Presentation),
    /// Open these files, each in a window of its own, and close this window: the welcome window's
    /// answer to a choice or a drop.
    OpenFiles(Vec<FilePath>),
    /// Show this file in the file manager (a notice's "Show in Folder").
    Reveal(FilePath),
    /// Open a web or mail address a link of the open file names, with the program that handles it.
    OpenUri(String),
    /// Install this tool through the system's package service; the answer is `Edge::helped`.
    Provide(Helper),
    /// The first file this window showed has loaded, and its content is naturally this size (a
    /// PDF's first page at 100%, a picture a plugin decoded). Sent once per window, never for a
    /// file the person moved on to: the host sizes the window to it if the person has not.
    SizeWindow(PixelSize),
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
    resume: Arc<dyn ResumeSource>,
    locks: Arc<dyn FileLocks>,
    versions: Arc<dyn VersionSource>,
    first_frames: Arc<dyn FirstFrameSource>,
    media: Arc<dyn MediaHost>,
    image_plugins: Arc<dyn ImagePlugins>,
    cards: Arc<dyn FileCards>,
    helpers: Arc<dyn HelperSource>,
    held: Arc<Mutex<Held>>,
}

/// The newest load that submitted work, and the stop its work shares. A job of a newer load
/// raises it, so what the person left behind (a search of the last query, an open of the last
/// file) ends early instead of running to its end.
#[derive(Debug, Default)]
struct Held {
    ticket: Ticket,
    stop: Stop,
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
            resume: Arc::new(Forgetful),
            locks: Arc::new(NoLocks),
            versions: Arc::new(NoVersions),
            first_frames: Arc::new(NoPictures),
            media: Arc::new(NoPlayer),
            image_plugins: Arc::new(NoImagePlugins),
            cards: Arc::new(NoCards),
            helpers: Arc::new(NoHelpers),
            held: Arc::default(),
        }
    }

    /// The same edge reading where files were left from `source`: without one nothing is
    /// remembered, and every file opens at its start.
    pub fn with_resume_source(self, source: Arc<dyn ResumeSource>) -> Edge {
        Edge {
            resume: source,
            ..self
        }
    }

    /// The same edge asking `locks` which files refuse a save in place: without it every file
    /// offers its edits.
    pub fn with_locks(self, locks: Arc<dyn FileLocks>) -> Edge {
        Edge { locks, ..self }
    }

    /// The same edge listing a file's kept versions from `source`: without one a file has none to
    /// go back to.
    pub fn with_version_source(self, source: Arc<dyn VersionSource>) -> Edge {
        Edge {
            versions: source,
            ..self
        }
    }

    /// The same edge showing the host's small pictures while a file opens: without them a file
    /// shows once it is open.
    pub fn with_first_frames(self, source: Arc<dyn FirstFrameSource>) -> Edge {
        Edge {
            first_frames: source,
            ..self
        }
    }

    /// The same edge starting its players with `host`: without one a recording does not open.
    pub fn with_media(self, host: Arc<dyn MediaHost>) -> Edge {
        Edge {
            media: host,
            ..self
        }
    }

    /// The same edge decoding pictures the viewer cannot (HEIC, raw files in full) through
    /// `plugins`: without them a file only a plugin can show is shown as its facts.
    pub fn with_image_plugins(self, plugins: Arc<dyn ImagePlugins>) -> Edge {
        Edge {
            image_plugins: plugins,
            ..self
        }
    }

    /// The same edge showing the files no stage covers (fonts, archives, folders, office documents)
    /// as `cards` describe them: without them a card lists only the kind and the size.
    pub fn with_cards(self, cards: Arc<dyn FileCards>) -> Edge {
        Edge { cards, ..self }
    }

    /// The same edge wording its install sheet from `helpers`: without them no tool is offered.
    pub fn with_helpers(self, helpers: Arc<dyn HelperSource>) -> Edge {
        Edge { helpers, ..self }
    }

    /// What the install sheet says of `helper`, or `None` when the host does not know the tool.
    pub fn helper_words(&self, helper: Helper) -> Option<super::helpers::HelperWords> {
        self.helpers.words(helper)
    }

    /// Tell the window that a tool a plugin runs is on the machine now (the person installed it
    /// some other way): a file that lacked it opens again. Callable from any thread, and a no-op
    /// once the window is gone.
    pub fn available(&self, helper: Helper) {
        self.reply.post(Done::Available(helper));
    }

    /// Tell the window how asking the system to install `helper` ended. Callable from any thread,
    /// and a no-op once the window is gone.
    pub fn helped(&self, helper: Helper, end: crate::HelperEnd) {
        self.reply.post(Done::Helped(helper, end));
    }

    /// Tell the window that `path` changed on disk. The window looks at the file's stamp and
    /// reloads it only if it differs from the one it opened, so a spurious event costs one stat.
    /// Callable from any thread, and a no-op once the window is gone.
    pub fn changed(&self, path: FilePath) {
        self.reply.post(Done::Changed { path });
    }

    /// Tell the window which files the person chose in the file dialog: they open as dropped
    /// files do. Callable from any thread, and a no-op once the window is gone.
    pub fn chosen(&self, files: Vec<FilePath>) {
        self.reply.post(Done::Chosen { files });
    }

    /// Tell the window the file it shows was renamed: the window opens it again under `to`,
    /// where the person is. Callable from any thread, and a no-op once the window is gone.
    pub fn moved(&self, to: FilePath) {
        self.reply.post(Done::Moved { to });
    }

    /// Tell the window how a task ended: it shows the notice for a moment. Callable from any
    /// thread, and a no-op once the window is gone.
    pub fn notify(&self, notice: Notice) {
        self.reply.post(Done::Notice(notice));
    }

    /// Ask a worker to do `job`; its result arrives in the mailbox.
    pub(crate) fn submit(&self, job: Job) {
        let job_ticket = job.ticket();
        self.workers.submit(Work {
            job,
            reply: self.reply.clone(),
            resume: Arc::clone(&self.resume),
            locks: Arc::clone(&self.locks),
            versions: Arc::clone(&self.versions),
            stop: self.stop_for(job_ticket),
        });
    }

    /// The stop of the load `ticket` names: its jobs share one, a newer load raises it, and a
    /// job of a load already left starts stopped. Work that belongs to no load is never stopped.
    fn stop_for(&self, ticket: Ticket) -> Stop {
        if ticket == Ticket::default() {
            return Stop::new();
        }
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if ticket < held.ticket {
            let late = Stop::new();
            late.request();
            return late;
        }
        if ticket > held.ticket {
            held.stop.request();
            *held = Held {
                ticket,
                stop: Stop::new(),
            };
        }
        held.stop.clone()
    }

    /// What an open of a file into `texture` needs.
    pub(crate) fn link(&self, texture: TextureHandle) -> OpenLink {
        OpenLink {
            media: Some(MediaPort {
                host: Arc::clone(&self.media),
                reply: self.reply.clone(),
            }),
            ..self.link_for_preload(texture)
        }
    }

    /// What opening a file ahead of the person needs: the same, but it can start no player.
    pub(crate) fn link_for_preload(&self, texture: TextureHandle) -> OpenLink {
        OpenLink {
            texture,
            highlighter: Arc::clone(&self.highlighter),
            first_frames: Arc::clone(&self.first_frames),
            image_plugins: Arc::clone(&self.image_plugins),
            cards: Arc::clone(&self.cards),
            media: None,
        }
    }

    /// The mailbox, once: the window's UI task takes it and awaits the results.
    pub(crate) fn take_mailbox(&self) -> Option<UnboundedReceiver<Done>> {
        self.mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Hand a request to the binary.
    pub(crate) fn request(&self, request: HostRequest) {
        (self.requests)(request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge() -> Edge {
        struct Nowhere;
        impl Workers for Nowhere {
            fn submit(&self, _work: Work) {}
        }
        Edge::new(Arc::new(Nowhere), |_| {})
    }

    #[test]
    fn a_newer_load_stops_the_work_of_the_one_it_replaced() {
        let edge = edge();
        let first = edge.stop_for(Ticket(1));
        let same = edge.stop_for(Ticket(1));
        assert_eq!(first.stopped(), StopState::Running);
        assert_eq!(same.stopped(), StopState::Running, "one load shares a stop");
        let second = edge.stop_for(Ticket(2));
        assert_eq!(first.stopped(), StopState::Stopped);
        assert_eq!(same.stopped(), StopState::Stopped);
        assert_eq!(second.stopped(), StopState::Running);
    }

    #[test]
    fn work_of_a_load_already_left_starts_stopped_and_loadless_work_never_is() {
        let edge = edge();
        let current = edge.stop_for(Ticket(5));
        assert_eq!(edge.stop_for(Ticket(4)).stopped(), StopState::Stopped);
        let ahead = edge.stop_for(Ticket::default());
        assert_eq!(ahead.stopped(), StopState::Running);
        assert_eq!(
            current.stopped(),
            StopState::Running,
            "loadless work stops no load"
        );
    }
}
