//! How work leaves the window and its result comes back. The window never runs a [`Job`] itself
//! and the library never spawns a thread: it hands each [`Work`] to the binary's [`Workers`]
//! (its one bounded pool), a worker calls [`Work::run`], and the [`Done`] it makes is posted to
//! the window's mailbox through the [`Reply`] the work carries. Results are machine inputs with
//! the load's ticket, so one that arrives after the person left the file is a listed no-op.

use super::job::{Done, Job, Keeping, OpenPort, Opened, WorkLane};
use super::media::{MediaPort, MediaSupport};
use super::notice::Notice;
use super::seam_set::Seams;
use super::seams::{FileLocks, ResumeSource, VersionSource};
use crate::{
    EditRequest, ExportDraft, LoadFailure, Presentation, Rewind, Ticket, TypedText, VersionKey,
};
use anyview_core::work::{Stop, StopState};
use anyview_core::{FileAction, FilePath, Helper, MarkupSave, PixelSize, Resume};
use anyview_machines::{PlatformAbilities, SaveEnd};
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::ops::{Deref, DerefMut};
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
    /// Search an open file for a phrase: a text, a table's cells, a tree's keys and values.
    Search,
    /// Sort a table sheet by a column.
    Sort,
    /// Read a text whole, to edit it.
    EditRead,
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
        let searching = job.searches();
        let ran = catch_unwind(AssertUnwindSafe(|| {
            job.run(resume.as_ref(), locks.as_ref(), versions.as_ref(), &stop)
        }));
        // A search that was replaced while it read has only some of the hits: nobody waits on it.
        if searching && stop.stopped() == StopState::Stopped {
            return;
        }
        reply.post(ran.unwrap_or(failure));
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
pub struct Reply(UnboundedSender<Done>);

impl Reply {
    /// Leave `done` in the mailbox. A window that closed has no mailbox, and the result is
    /// dropped: nobody is waiting for it.
    pub fn post(&self, done: Done) {
        let _closed = self.0.unbounded_send(done);
    }
}

/// What a content's natural size is measured in, so the window can turn it into logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalSize {
    /// Device pixels, one to one: a picture. On a 2x output it is half as many logical pixels.
    Pixels(PixelSize),
    /// Logical pixels as they are: a PDF's page at 100%, where a point is a logical pixel.
    Points(PixelSize),
    /// Logical pixels as they are, for a window that holds a player and little else (an audio
    /// file): it may be smaller than the least a picture's window is.
    Compact(PixelSize),
}

/// What a different file sizes the window to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeBasis {
    /// The loaded document says its content is this size.
    Natural(NaturalSize),
    /// The document loaded and knows no size of its own (a recording's picture, a picture whose
    /// plugin is missing): the host may read the size from the file's header, off the UI thread.
    Header,
    /// The file did not load: the default window, as for any file with no natural size.
    Default,
}

/// What the window asks of the binary that hosts it: the things the viewer decides to do but
/// cannot do itself (they touch the platform, the file system or the window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRequest {
    /// The window now shows this file: the host records it as viewed, and it is the file the
    /// requests below that name none refer to.
    Opened(Opened),
    /// The file the window was asked to show could not be opened, and the window now says so.
    /// See [`NotOpened`] for when it is sent.
    NotOpened(NotOpened),
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
    /// Write the open text file in place with this text, as the person edited it; the host keeps
    /// the original first. Its end is told to the window (`Edge::saved`).
    SaveText(TextSave),
    /// Write the open PDF in place with the form fields filled and the marks made on it, appended
    /// to the file; the host keeps the original first.
    SaveMarkup(MarkupSave),
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
    /// A different file has loaded (or failed to), and this is what to size the window to. Sent
    /// once for each file the window opens, never for the same file loaded again: the host sizes
    /// the window to it, as it did when the first file opened.
    SizeWindow(SizeBasis),
}

/// A file the viewer was asked to show and could not open: the host may offer another way to
/// open it (another program's, say), since the viewer has none.
///
/// Sent once for each open that fails, when the viewer's own failure card goes up: whether the
/// file could not be read, was damaged, locked, empty, over what the viewer opens or of a kind it
/// has no way to show. A file that opens as facts only (an archive, a font, a recording without a
/// player) did open, and is announced by [`HostRequest::Opened`] instead. A window shown again
/// for the same file (it changed on disk and the new copy fails) sends it again for that load,
/// never for a redraw. The next [`HostRequest::Opened`] supersedes it: a later load of the same
/// file that succeeds announces the file as opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotOpened {
    /// The file that did not open.
    pub file: FilePath,
    /// Why, as the failure card says it.
    pub reason: LoadFailure,
}

/// The text of the open file as the person edited it, whole, as the file is to hold it: its
/// line endings and byte-order mark are the ones it had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSave {
    bytes: Vec<u8>,
}

impl TextSave {
    /// The file's new bytes.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> TextSave {
        TextSave { bytes }
    }

    /// The bytes to write.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
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
    seams: Seams,
    held: Arc<Mutex<Held>>,
}

/// The receiving end of an edge's results, held by one task at a time and given back to the edge
/// when it is dropped.
pub(crate) struct Mailbox {
    home: Arc<Mutex<Option<UnboundedReceiver<Done>>>>,
    receiver: UnboundedReceiver<Done>,
}

impl Deref for Mailbox {
    type Target = UnboundedReceiver<Done>;

    fn deref(&self) -> &UnboundedReceiver<Done> {
        &self.receiver
    }
}

impl DerefMut for Mailbox {
    fn deref_mut(&mut self) -> &mut UnboundedReceiver<Done> {
        &mut self.receiver
    }
}

impl Drop for Mailbox {
    fn drop(&mut self) {
        let (_, spare) = unbounded();
        let receiver = std::mem::replace(&mut self.receiver, spare);
        *self.home.lock().unwrap_or_else(PoisonError::into_inner) = Some(receiver);
    }
}

/// The newest load that submitted work, and the stop its work shares. A job of a newer load
/// raises it, so what the person left behind (a search of the last query, an open of the last
/// file) ends early instead of running to its end. The newest search of the load has a stop of
/// its own, which the next search of the load raises: a query typed over an older one ends the
/// older one's reading.
#[derive(Debug, Default)]
struct Held {
    ticket: Ticket,
    stop: Stop,
    search: Stop,
}

impl std::fmt::Debug for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Edge").finish_non_exhaustive()
    }
}

impl Edge {
    /// An edge over `workers`, handing the window's requests to `on_request`, with the host's
    /// `seams`.
    #[must_use]
    pub fn new(
        workers: Arc<dyn Workers>,
        on_request: impl Fn(HostRequest) + Send + Sync + 'static,
        seams: Seams,
    ) -> Edge {
        let (sender, receiver) = unbounded();
        Edge {
            workers,
            reply: Reply(sender),
            mailbox: Arc::new(Mutex::new(Some(receiver))),
            requests: Arc::new(on_request),
            highlighter: Arc::new(Highlighter::new()),
            seams,
            held: Arc::default(),
        }
    }

    /// Whether the host this edge was built with starts players, for a pane to decide whether a
    /// recording plays in it or is handed to the host.
    #[must_use]
    pub fn media_support(&self) -> MediaSupport {
        self.seams.media.support()
    }

    /// What the platform can do, for the window to decide what to offer.
    #[must_use]
    pub fn platform(&self) -> PlatformAbilities {
        self.seams.platform
    }

    /// What the install sheet says of `helper`, or `None` when the host does not know the tool.
    #[must_use]
    pub fn helper_words(&self, helper: Helper) -> Option<super::helpers::HelperWords> {
        self.seams.helpers.words(helper)
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

    /// Tell the window how a save of the edited text of `path` ended. Callable from any thread,
    /// and a no-op once the window is gone.
    pub fn saved(&self, path: FilePath, end: SaveEnd) {
        self.reply.post(Done::Saved { path, end });
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

    /// Whether this edge was given a keeper, so the window writes the views and places itself.
    #[must_use]
    pub fn keeps(&self) -> bool {
        self.seams.keeper.is_some()
    }

    /// Have a worker write `keeping` to the keeper the seams gave. Nothing is written when
    /// there is none. Callable from any thread.
    pub fn keep(&self, keeping: Keeping) {
        if let Some(keeper) = &self.seams.keeper {
            self.submit(Job::Keep {
                keeper: Arc::clone(keeper),
                keeping,
            });
        }
    }

    /// Ask a worker to do `job`; its result arrives in the mailbox.
    pub(crate) fn submit(&self, job: Job) {
        let job_ticket = job.ticket();
        let stop = self.stop_for_job(job_ticket, job.searches());
        self.workers.submit(Work {
            job,
            reply: self.reply.clone(),
            resume: Arc::clone(&self.seams.resume),
            locks: Arc::clone(&self.seams.locks),
            versions: Arc::clone(&self.seams.versions),
            stop,
        });
    }

    /// The stop of the load `ticket` names: its jobs share one, a newer load raises it, and a
    /// job of a load already left starts stopped. Work that belongs to no load is never stopped.
    #[cfg(test)]
    fn stop_for(&self, ticket: Ticket) -> Stop {
        self.stop_for_job(ticket, false)
    }

    /// The stop of a job of the load `ticket`: the load's, or for a search `searching` its own,
    /// which ends with the load and when a newer search of the load is asked for.
    fn stop_for_job(&self, ticket: Ticket, searching: bool) -> Stop {
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
            held.search.request();
            *held = Held {
                ticket,
                stop: Stop::new(),
                search: Stop::new(),
            };
        }
        if !searching {
            return held.stop.clone();
        }
        held.search.request();
        held.search = Stop::new();
        held.search.clone()
    }

    /// What an open of a file into `texture` needs.
    pub(crate) fn link(&self, texture: TextureHandle) -> OpenPort {
        OpenPort {
            media: Some(MediaPort {
                host: Arc::clone(&self.seams.media),
                reply: self.reply.clone(),
            }),
            ..self.link_for_preload(texture)
        }
    }

    /// What opening a file ahead of the person needs: the same, but it can start no player.
    pub(crate) fn link_for_preload(&self, texture: TextureHandle) -> OpenPort {
        OpenPort {
            texture,
            highlighter: Arc::clone(&self.highlighter),
            first_frames: Arc::clone(&self.seams.first_frames),
            image_plugins: Arc::clone(&self.seams.image_plugins),
            media: None,
        }
    }

    /// The mailbox, for as long as the caller holds it: the window's UI task takes it and awaits
    /// the results. A clone of the edge shares it, so only one holder has it at a time; when a
    /// holder is dropped (the window's task ended, the view mounted again) the mailbox is back
    /// and the next `take_mailbox` gets it, with whatever arrived in between.
    pub(crate) fn take_mailbox(&self) -> Option<Mailbox> {
        let receiver = self
            .mailbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        Some(Mailbox {
            home: Arc::clone(&self.mailbox),
            receiver,
        })
    }

    /// Hand a request to the binary.
    pub(crate) fn request(&self, request: HostRequest) {
        (self.requests)(request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{OpenError, ResumeSource};
    use anyview_core::FileStamp;

    /// A host store that falls over when asked, as a decoder does on a hostile file.
    #[derive(Debug)]
    struct Falling;

    impl ResumeSource for Falling {
        fn recall(&self, _path: &FilePath, _stamp: FileStamp) -> Resume {
            panic!("the store fell over");
        }
    }

    /// A pool of no threads: the work runs where it is submitted.
    struct Inline;

    impl Workers for Inline {
        fn submit(&self, work: Work) {
            work.run();
        }
    }

    #[test]
    fn a_job_that_panics_answers_its_load_with_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, b"hello\n").unwrap();
        let path = FilePath::new(&file).unwrap();
        let seams = Seams::default().with_resume_source(Arc::new(Falling));
        let edge = Edge::new(Arc::new(Inline), |_| {}, seams);
        edge.submit(Job::Probe {
            ticket: Ticket(1),
            path,
        });
        let mut mailbox = edge.take_mailbox().unwrap();
        let Ok(Done::Probed { ticket, result }) = mailbox.try_recv() else {
            panic!("the abandoned probe left no answer");
        };
        assert_eq!(ticket, Ticket(1));
        let error = result.unwrap_err();
        assert!(matches!(error, OpenError::Crashed));
        assert_eq!(error.failure(), LoadFailure::Damaged);
    }

    fn edge() -> Edge {
        struct Nowhere;
        impl Workers for Nowhere {
            fn submit(&self, _work: Work) {}
        }
        Edge::new(Arc::new(Nowhere), |_| {}, Seams::default())
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
    fn a_newer_search_stops_the_older_one_and_a_newer_load_stops_the_search() {
        let edge = edge();
        let load = edge.stop_for_job(Ticket(1), false);
        let older = edge.stop_for_job(Ticket(1), true);
        assert_eq!(older.stopped(), StopState::Running);
        let newer = edge.stop_for_job(Ticket(1), true);
        assert_eq!(
            older.stopped(),
            StopState::Stopped,
            "a newer query replaced it"
        );
        assert_eq!(newer.stopped(), StopState::Running);
        assert_eq!(load.stopped(), StopState::Running, "the load goes on");
        edge.stop_for_job(Ticket(2), false);
        assert_eq!(
            newer.stopped(),
            StopState::Stopped,
            "a newer load ends its searches"
        );
        assert_eq!(
            edge.stop_for_job(Ticket(1), true).stopped(),
            StopState::Stopped,
            "a search of a load already left starts stopped"
        );
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
