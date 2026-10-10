//! The pane's own memory of what the person viewed and where they left each file: the viewer's
//! store (`anyview-store`) opened at a root the host chose, so a pane and the viewer's own windows
//! (or two panes in two programs) share one history without erasing each other's entries.

use anyview_core::{FilePath, FileStamp, Resume, Source};
use anyview_store::{Clock, HistoryCap, StoreWriter};
use anyview_ui::{
    Edge, HostRequest, Keeping, Noted, Opened, REMEMBER_EVERY, Remembering, ResumeKeeper,
    ResumeSource,
};
use ds::base::time::clock;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The store at one root, read when a file opens and written by the host's workers.
#[derive(Clone)]
pub(crate) struct PaneStore {
    writer: StoreWriter,
    /// What each recorded view is stamped with.
    now: Clock,
}

impl std::fmt::Debug for PaneStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneStore")
            .field("writer", &self.writer)
            .finish_non_exhaustive()
    }
}

impl PaneStore {
    pub(crate) fn at(root: PathBuf, now: Clock) -> PaneStore {
        PaneStore {
            writer: StoreWriter::new(root, HistoryCap::DEFAULT),
            now,
        }
    }
}

impl ResumeSource for PaneStore {
    fn recall(&self, path: &FilePath, stamp: FileStamp) -> Resume {
        self.writer
            .load_resume(path, stamp)
            .ok()
            .flatten()
            .unwrap_or(Resume::Nothing)
    }
}

impl ResumeKeeper for PaneStore {
    fn viewed(&self, opened: &Opened) {
        if let Err(error) = self.writer.record_view(
            opened.source.path(),
            opened.sniffed.kind(),
            (self.now)(),
            &Resume::Nothing,
        ) {
            eprintln!("anyview: cannot record the view: {error}");
        }
    }

    fn remember(&self, source: &Source, resume: &Resume) {
        if let Err(error) = self
            .writer
            .save_resume(source.path(), source.stamp(), resume)
        {
            eprintln!("anyview: cannot keep the place: {error}");
        }
    }
}

/// What one pane has told the store so far: the file it shows, and the places the viewer's one
/// remembering policy ([`Remembering`]) has let through. A place is written at most once in
/// [`REMEMBER_EVERY`], and the latest one held back is written when the file is let go of
/// ([`Kept::finish`]), so the last place is never lost. A place that has to wait is written when
/// its interval ends ([`Kept::due_in`], then [`Kept::wake`]), on the clock of
/// `ds_core::time::clock`, which the pane's component sleeps on.
#[derive(Debug)]
pub(crate) struct Kept {
    open: Option<Source>,
    policy: Remembering,
    /// Zero of the clock the policy is told the time on.
    origin: Instant,
    /// When the place held back is due, on the policy's clock; none when nothing is held back.
    due: Option<Duration>,
}

impl Default for Kept {
    fn default() -> Kept {
        Kept {
            open: None,
            policy: Remembering::new(REMEMBER_EVERY),
            origin: clock::now(),
            due: None,
        }
    }
}

impl Kept {
    /// The pane is done with its file (another opened, or the pane is going away): write the place
    /// held back, on the edge's workers.
    pub(crate) fn finish(&mut self, edge: &Edge) {
        self.due = None;
        if let Some(source) = self.open.take()
            && let Some((source, resume)) = self.policy.release(source.path())
        {
            edge.keep(Keeping::Place { source, resume });
        }
    }

    /// How long until the place held back is due, if one is held.
    pub(crate) fn due_in(&self) -> Option<Duration> {
        self.due
            .map(|due| due.saturating_sub(clock::since(self.origin)))
    }

    /// The interval of the place held back has ended: write it, on the edge's workers.
    pub(crate) fn wake(&mut self, edge: &Edge) {
        self.due = None;
        if let Some(source) = &self.open
            && let Some((source, resume)) =
                self.policy.take(source.path(), clock::since(self.origin))
        {
            edge.keep(Keeping::Place { source, resume });
        }
    }

    /// Hand what `request` asks the pane to keep to the edge's workers. True when the store
    /// answered the request, so the host is not asked; the host still hears that a file opened.
    /// An edge with no store answers nothing.
    pub(crate) fn answers(&mut self, request: &HostRequest, edge: &Edge) -> bool {
        if !edge.keeps() {
            return false;
        }
        match request {
            HostRequest::Opened(opened) => {
                self.finish(edge);
                self.open = Some(opened.source.clone());
                edge.keep(Keeping::Viewed(opened.clone()));
                false
            }
            HostRequest::Remember(resume) => {
                if let Some(source) = &self.open {
                    match self.policy.note(
                        source.clone(),
                        resume.clone(),
                        clock::since(self.origin),
                    ) {
                        Noted::Write(source, resume) => {
                            self.due = None;
                            edge.keep(Keeping::Place { source, resume });
                        }
                        Noted::Wait { due } => self.due = Some(due),
                        Noted::Unchanged | Noted::Replaced => {}
                    }
                }
                true
            }
            HostRequest::NotOpened(_)
            | HostRequest::Run(_)
            | HostRequest::PickFile
            | HostRequest::CloseWindow
            | HostRequest::Export(_)
            | HostRequest::Trash
            | HostRequest::Rename(_)
            | HostRequest::Edit(_)
            | HostRequest::SaveText(_)
            | HostRequest::Rewind(_)
            | HostRequest::RevertTo(_)
            | HostRequest::SaveCopy(_)
            | HostRequest::Watch(_)
            | HostRequest::Unwatch
            | HostRequest::Present(_)
            | HostRequest::OpenFiles(_)
            | HostRequest::Reveal(_)
            | HostRequest::OpenUri(_)
            | HostRequest::Provide(_)
            | HostRequest::SizeWindow(_) => false,
        }
    }
}
