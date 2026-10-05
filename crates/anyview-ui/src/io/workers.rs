//! How work leaves the window and its result comes back. The window never runs a [`Job`] itself
//! and the library never spawns a thread: it hands each [`Work`] to the binary's [`Workers`]
//! (its one bounded pool), a worker calls [`Work::run`], and the [`Done`] it makes is posted to
//! the window's mailbox through the [`Reply`] the work carries. Results are machine inputs with
//! the load's ticket, so one that arrives after the person left the file is a listed no-op.

use super::job::{Done, Job, OpenLink, Probed, WorkLane};
use super::media::{MediaHost, MediaPort, NoPlayer};
use super::seams::{
    FirstFrameSource, Forgetful, ImagePlugins, NoImagePlugins, NoPictures, NoVersions,
    ResumeSource, VersionSource,
};
use crate::edits::{EditRequest, Rewind};
use crate::sheet::{ExportDraft, VersionKey};
use crate::{Presentation, Ticket, TypedText};
use anyview_core::{FileAction, FilePath, Resume};
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
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
    versions: Arc<dyn VersionSource>,
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

    /// Do the job, blocking, and post what it made to the window that asked.
    pub fn run(self) {
        let Work {
            job,
            reply,
            resume,
            versions,
        } = self;
        reply.post(job.run(resume.as_ref(), versions.as_ref()));
    }

    /// What the work does.
    pub fn kind(&self) -> WorkKind {
        match &self.job {
            Job::Probe { .. } => WorkKind::Probe,
            Job::Peek { .. } => WorkKind::Peek,
            Job::Open { .. } => WorkKind::Open,
            Job::Lines { .. } => WorkKind::Lines,
            Job::Search { .. } => WorkKind::Search,
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
    /// Open a web or mail address a link of the open file names, with the program that handles it.
    OpenUri(String),
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
    versions: Arc<dyn VersionSource>,
    first_frames: Arc<dyn FirstFrameSource>,
    media: Arc<dyn MediaHost>,
    image_plugins: Arc<dyn ImagePlugins>,
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
            versions: Arc::new(NoVersions),
            first_frames: Arc::new(NoPictures),
            media: Arc::new(NoPlayer),
            image_plugins: Arc::new(NoImagePlugins),
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

    /// Tell the window that `path` changed on disk. The window looks at the file's stamp and
    /// reloads it only if it differs from the one it opened, so a spurious event costs one stat.
    /// Callable from any thread, and a no-op once the window is gone.
    pub fn changed(&self, path: FilePath) {
        self.reply.post(Done::Changed { path });
    }

    /// Ask a worker to do `job`; its result arrives in the mailbox.
    pub(crate) fn submit(&self, job: Job) {
        self.workers.submit(Work {
            job,
            reply: self.reply.clone(),
            resume: Arc::clone(&self.resume),
            versions: Arc::clone(&self.versions),
        });
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
