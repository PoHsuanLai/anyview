//! What one viewer window is wired to: the workers, the mailbox the results come back in, and
//! the binary's requests.

use super::super::job::{Done, Job, Keeping, OpenPort, Replaces};
use super::super::media::{MediaPort, MediaSupport};
use super::super::notice::Notice;
use super::super::seam_set::Seams;
use super::requests::HostRequest;
use super::work::{Reply, Work, Workers};
use crate::Ticket;
use anyview_core::work::Stop;
use anyview_core::{FilePath, Helper};
use anyview_machines::{PlatformAbilities, SaveEnd};
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, PoisonError};

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
/// older one's reading. The newest window of lines is the same: one scrolled past is not read.
#[derive(Debug, Default)]
struct Held {
    ticket: Ticket,
    stop: Stop,
    search: Stop,
    lines: Stop,
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
    pub fn helper_words(&self, helper: Helper) -> Option<super::super::helpers::HelperWords> {
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

    /// The width the person left the left panel at last time, in logical pixels, when the host
    /// remembers one.
    #[must_use]
    pub fn panel_width(&self) -> Option<u32> {
        self.seams.panel.recall()
    }

    /// The person left the left panel `width` logical pixels wide: the host keeps it.
    pub fn keep_panel_width(&self, width: u32) {
        self.seams.panel.keep(width);
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
        let stop = self.stop_for_job(job_ticket, job.replaces());
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
        self.stop_for_job(ticket, Replaces::Nothing)
    }

    /// The stop of a job of the load `ticket`: the load's, or for a search or a window of lines
    /// (`replaces`) its own, which ends with the load and when a newer job of that kind is asked
    /// for.
    fn stop_for_job(&self, ticket: Ticket, replaces: Replaces) -> Stop {
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
            held.lines.request();
            *held = Held {
                ticket,
                stop: Stop::new(),
                search: Stop::new(),
                lines: Stop::new(),
            };
        }
        let own = match replaces {
            Replaces::Nothing => return held.stop.clone(),
            Replaces::Search => &mut held.search,
            Replaces::Lines => &mut held.lines,
        };
        own.request();
        *own = Stop::new();
        own.clone()
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
