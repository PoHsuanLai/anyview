//! The pane's own memory of what the person viewed and where they left each file: the viewer's
//! store (`anyview-store`) opened at a root the host chose, so a pane and the viewer's own windows
//! (or two panes in two programs) share one history without erasing each other's entries.

use anyview_core::{FilePath, FileStamp, Resume, Source};
use anyview_store::{HistoryCap, StoreWriter, Viewed};
use anyview_ui::{Edge, HostRequest, Keeping, Opened, ResumeKeeper, ResumeSource};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Seconds since the Unix epoch; a clock before it reads as the epoch itself.
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

/// What one pane has told the store so far: the file it shows, and the last place written for it.
#[derive(Debug, Default)]
pub(crate) struct Kept {
    open: Option<Source>,
    last: Option<Resume>,
}

impl Kept {
    /// Hand what `request` asks the pane to keep to the edge's workers. True when the store
    /// answered the request, so the host is not asked; the host still hears that a file opened.
    /// An edge with no store answers nothing.
    pub(crate) fn answers(&mut self, request: &HostRequest, edge: &Edge) -> bool {
        if !edge.keeps() {
            return false;
        }
        match request {
            HostRequest::Opened(opened) => {
                self.open = Some(opened.source.clone());
                self.last = None;
                edge.keep(Keeping::Viewed(opened.clone()));
                false
            }
            HostRequest::Remember(resume) => {
                if let Some(source) = &self.open
                    && self.last.as_ref() != Some(resume)
                {
                    self.last = Some(resume.clone());
                    edge.keep(Keeping::Place {
                        source: source.clone(),
                        resume: resume.clone(),
                    });
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
