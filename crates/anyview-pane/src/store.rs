//! The pane's own memory of what the person viewed and where they left each file: the viewer's
//! store (`anyview-store`) opened at a root the host chose, so a pane and the viewer's own windows
//! (or two panes in two programs) share one history without erasing each other's entries.

use anyview_core::{FilePath, FileStamp, Resume, Source};
use anyview_store::{HistoryCap, StoreWriter, Viewed};
use anyview_ui::{
    Edge, HostRequest, Keeping, Noted, Opened, REMEMBER_EVERY, Remembering, ResumeKeeper,
    ResumeSource,
};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The store at one root, read when a file opens and written by the host's workers.
#[derive(Debug, Clone)]
pub(crate) struct PaneStore {
    writer: StoreWriter,
}

impl PaneStore {
    pub(crate) fn at(root: PathBuf) -> PaneStore {
        PaneStore {
            writer: StoreWriter::new(root, HistoryCap::DEFAULT),
        }
    }
}

/// Seconds since the Unix epoch, on the system clock: an edge has no clock of the host's to stamp a
/// view with. A clock before it reads as the epoch itself.
fn now() -> Viewed {
    Viewed(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    )
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
            now(),
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
/// ([`Kept::finish`]), so the last place is never lost. The pane has no timer: a place that has to
/// wait is written by the next one after the interval, or by the finish.
#[derive(Debug)]
pub(crate) struct Kept {
    open: Option<Source>,
    policy: Remembering,
    /// Zero of the clock the policy is told the time on.
    origin: Instant,
}

impl Default for Kept {
    fn default() -> Kept {
        Kept {
            open: None,
            policy: Remembering::new(REMEMBER_EVERY),
            origin: Instant::now(),
        }
    }
}

impl Kept {
    /// The pane is done with its file (another opened, or the pane is going away): write the place
    /// held back, on the edge's workers.
    pub(crate) fn finish(&mut self, edge: &Edge) {
        if let Some(source) = self.open.take()
            && let Some((source, resume)) = self.policy.release(source.path())
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
                if let Some(source) = &self.open
                    && let Noted::Write(source, resume) =
                        self.policy
                            .note(source.clone(), resume.clone(), self.origin.elapsed())
                {
                    edge.keep(Keeping::Place { source, resume });
                }
                true
            }
            HostRequest::Run(_)
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
