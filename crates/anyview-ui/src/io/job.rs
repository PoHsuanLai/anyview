//! The work a window hands its workers, and what comes back. Each job blocks (disk, decoding,
//! highlighting); none touches the UI thread's state. A job that shows pixels uploads them into
//! the texture it was given itself, so a decoded picture never passes through the UI thread: its
//! result is a small document that names the texture.

use super::error::OpenError;
use super::folder::folder_sequence;
use super::seams::{FirstFrameSource, ImagePlugins, ResumeSource, VersionSource};
use crate::families::{
    BookDoc, FoundHits, LineWindow, LoadedDoc, PdfAnswer, PdfTask, SectionPage, TextDoc, open_for,
    peek_for,
};
use crate::sheet::VersionRow;
use crate::{StageFamily, Ticket, TypedText};
use anyview_core::work::Stop;
use anyview_core::{
    FilePath, FileStamp, LineIndex, Resume, SectionIndex, Sequence, Sniffed, Source,
};
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use std::sync::Arc;

/// The largest file a preload opens: the next file is worth opening ahead only while that is
/// cheap, and a file this large is better opened when it is asked for.
const PRELOAD_LIMIT: u64 = 64 * 1024 * 1024;

/// What an open needs from the window: the texture a picture is uploaded into (one per open, made
/// on the UI thread from the window's `Gpu`), the one highlighter every file shares, and the
/// pictures the host has ready.
#[derive(Debug, Clone)]
pub struct OpenLink {
    /// Where a decoded picture goes.
    pub texture: TextureHandle,
    /// Highlights code for every stage.
    pub highlighter: Arc<Highlighter>,
    /// The host's small pictures, for a first frame.
    pub first_frames: Arc<dyn FirstFrameSource>,
    /// The plugins that decode what the viewer cannot.
    pub image_plugins: Arc<dyn ImagePlugins>,
    /// The host's players, when this open may start one: a file opened ahead of the person never
    /// plays, so a preload carries none.
    pub(crate) media: Option<super::MediaPort>,
}

/// A file whose type was read: what to open, how it was sniffed, which stage shows it and where
/// the person left it last time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    /// The file and its stamp.
    pub source: Source,
    /// Proof the type was sniffed.
    pub sniffed: Sniffed,
    /// The stage that shows it.
    pub family: StageFamily,
    /// Where it was left, as the host's store remembered it for this version of the file.
    pub resume: Resume,
}

/// A file opened ahead of the person asking for it.
#[derive(Debug, Clone)]
pub struct Preloaded {
    /// What was probed.
    pub probed: Probed,
    /// The open document.
    pub doc: LoadedDoc,
}

/// How soon a job is wanted, for a pool that runs the nearest first: what the person is looking
/// at now is `Visible`, what is read ahead of them is `Preload`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkLane {
    /// Wanted now.
    Visible,
    /// Wanted soon, if at all.
    Preload,
}

/// One unit of blocking work.
#[derive(Debug)]
pub enum Job {
    /// Find out what the file of this load is.
    Probe { ticket: Ticket, path: FilePath },
    /// Make the cheap first frame of the probed file.
    Peek {
        ticket: Ticket,
        probed: Probed,
        link: OpenLink,
    },
    /// Open the probed file in full.
    Open {
        ticket: Ticket,
        probed: Probed,
        link: OpenLink,
    },
    /// Read and highlight `rows` lines of an open text file from `first`.
    Lines {
        ticket: Ticket,
        doc: Arc<TextDoc>,
        first: LineIndex,
        rows: u32,
    },
    /// Find every place `query` occurs in an open text file.
    Search {
        ticket: Ticket,
        doc: Arc<TextDoc>,
        query: TypedText,
    },
    /// Unpack and seal one section of an open book.
    Section {
        ticket: Ticket,
        doc: Arc<BookDoc>,
        section: SectionIndex,
    },
    /// Probe and open the file `path`, for the person to arrive at it without waiting.
    Preload { path: FilePath, link: OpenLink },
    /// Read the stamp `path` has now.
    Stat { path: FilePath },
    /// List the folder `path` is in, as the sequence the arrow keys walk.
    Folder { path: FilePath },
    /// Draw tiles, a thumbnail or a search of an open PDF.
    Pdf(PdfTask),
    /// List the versions kept of the file `path`, for the Revert To sheet.
    Versions { path: FilePath },
}

/// What a worker made of a job.
#[derive(Debug)]
pub enum Done {
    /// The probe of `ticket`.
    Probed {
        ticket: Ticket,
        result: Result<Probed, OpenError>,
    },
    /// The first frame of `ticket`, or `None` when the file has no cheap one.
    Peeked {
        ticket: Ticket,
        result: Result<Option<LoadedDoc>, OpenError>,
    },
    /// The open of `ticket`.
    Opened {
        ticket: Ticket,
        result: Result<LoadedDoc, OpenError>,
    },
    /// A window of lines of the file of `ticket`.
    Lines {
        ticket: Ticket,
        result: Result<LineWindow, OpenError>,
    },
    /// The hits of `query` in the file of `ticket`.
    Found {
        ticket: Ticket,
        query: TypedText,
        result: Result<FoundHits, OpenError>,
    },
    /// A section of the book of `ticket`.
    Section {
        ticket: Ticket,
        result: Result<SectionPage, OpenError>,
    },
    /// The file `path`, opened ahead; `None` when it could not be or was too large to be worth it.
    Preloaded {
        path: FilePath,
        loaded: Option<Preloaded>,
    },
    /// The stamp `path` has now; `None` when it cannot be read.
    Stamped {
        path: FilePath,
        stamp: Option<FileStamp>,
    },
    /// The folder of `path` as a sequence.
    Folder {
        path: FilePath,
        result: Result<Sequence, OpenError>,
    },
    /// The host saw `path` change on disk (`Edge::changed`).
    Changed { path: FilePath },
    /// What a PDF task made, for the load that asked.
    Pdf { ticket: Ticket, answer: PdfAnswer },
    /// The player of the load `ticket` has news: the window drains its line.
    Media { ticket: Ticket },
    /// The versions kept of `path`, newest first.
    Versions {
        path: FilePath,
        rows: Vec<VersionRow>,
    },
}

impl Job {
    /// How soon the job is wanted: a pool that has a queue for each runs the nearest first.
    pub fn lane(&self) -> WorkLane {
        match self {
            Job::Probe { .. }
            | Job::Peek { .. }
            | Job::Open { .. }
            | Job::Lines { .. }
            | Job::Search { .. }
            | Job::Section { .. }
            | Job::Stat { .. }
            | Job::Folder { .. }
            | Job::Versions { .. } => WorkLane::Visible,
            Job::Preload { .. } => WorkLane::Preload,
            Job::Pdf(task) => task.lane(),
        }
    }

    /// The load this job belongs to; `Ticket::default()` for a job that belongs to none.
    pub fn ticket(&self) -> Ticket {
        match self {
            Job::Probe { ticket, .. }
            | Job::Peek { ticket, .. }
            | Job::Open { ticket, .. }
            | Job::Lines { ticket, .. }
            | Job::Search { ticket, .. }
            | Job::Section { ticket, .. } => *ticket,
            Job::Pdf(task) => task.ticket(),
            Job::Preload { .. } | Job::Stat { .. } | Job::Folder { .. } | Job::Versions { .. } => {
                Ticket::default()
            }
        }
    }

    /// What a job that panicked answers, so the window that waits for it hears of the failure: the
    /// same message its own error would have been.
    pub(super) fn crashed(&self) -> Done {
        match self {
            Job::Probe { ticket, .. } => Done::Probed {
                ticket: *ticket,
                result: Err(OpenError::Crashed),
            },
            Job::Peek { ticket, .. } => Done::Peeked {
                ticket: *ticket,
                result: Err(OpenError::Crashed),
            },
            Job::Open { ticket, .. } => Done::Opened {
                ticket: *ticket,
                result: Err(OpenError::Crashed),
            },
            Job::Lines { ticket, .. } => Done::Lines {
                ticket: *ticket,
                result: Err(OpenError::Crashed),
            },
            Job::Search { ticket, query, .. } => Done::Found {
                ticket: *ticket,
                query: query.clone(),
                result: Err(OpenError::Crashed),
            },
            Job::Section { ticket, .. } => Done::Section {
                ticket: *ticket,
                result: Err(OpenError::Crashed),
            },
            Job::Preload { path, .. } => Done::Preloaded {
                path: path.clone(),
                loaded: None,
            },
            Job::Stat { path } => Done::Stamped {
                path: path.clone(),
                stamp: None,
            },
            Job::Folder { path } => Done::Folder {
                path: path.clone(),
                result: Err(OpenError::Crashed),
            },
            Job::Pdf(task) => Done::Pdf {
                ticket: task.ticket(),
                answer: task.crashed(),
            },
            Job::Versions { path } => Done::Versions {
                path: path.clone(),
                rows: Vec::new(),
            },
        }
    }

    /// Do the work, blocking until it is done. `resume` is where the host keeps view memory.
    pub(super) fn run(
        self,
        resume: &dyn ResumeSource,
        versions: &dyn VersionSource,
        stop: &Stop,
    ) -> Done {
        match self {
            Job::Probe { ticket, path } => Done::Probed {
                ticket,
                result: probed(&path, resume),
            },
            Job::Peek {
                ticket,
                probed,
                link,
            } => Done::Peeked {
                ticket,
                result: peek_for(ticket, &probed.source, &probed.sniffed, &link),
            },
            Job::Open {
                ticket,
                probed,
                link,
            } => Done::Opened {
                ticket,
                result: open_for(ticket, &probed.source, &probed.sniffed, &link),
            },
            Job::Lines {
                ticket,
                doc,
                first,
                rows,
            } => Done::Lines {
                ticket,
                result: doc.window(first, rows).map_err(OpenError::from),
            },
            Job::Search { ticket, doc, query } => Done::Found {
                ticket,
                result: doc.find(&query, stop).map_err(OpenError::from),
                query,
            },
            Job::Section {
                ticket,
                doc,
                section,
            } => Done::Section {
                ticket,
                result: doc.section(section).map_err(OpenError::from),
            },
            Job::Preload { path, link } => Done::Preloaded {
                loaded: preloaded(&path, &link, resume),
                path,
            },
            Job::Stat { path } => Done::Stamped {
                stamp: super::probe::stamp_of(&path),
                path,
            },
            Job::Folder { path } => Done::Folder {
                result: folder_sequence(&path),
                path,
            },
            Job::Pdf(task) => Done::Pdf {
                ticket: task.ticket(),
                answer: task.run(),
            },
            Job::Versions { path } => Done::Versions {
                rows: versions.list(&path),
                path,
            },
        }
    }
}

/// The probe of `path`, with what the host remembers of it.
fn probed(path: &FilePath, resume: &dyn ResumeSource) -> Result<Probed, OpenError> {
    let probed = super::probe(path)?;
    let remembered = resume.recall(path, probed.source.stamp());
    Ok(Probed {
        resume: remembered,
        ..probed
    })
}

/// `path` opened, or `None` when it is too large, cannot be opened or opens to nothing the
/// person will not meet again by asking: a failed preload is silent, the open when asked reports.
fn preloaded(path: &FilePath, link: &OpenLink, resume: &dyn ResumeSource) -> Option<Preloaded> {
    let probed = probed(path, resume).ok()?;
    if probed.source.stamp().len.0 > PRELOAD_LIMIT {
        return None;
    }
    let doc = open_for(Ticket::default(), &probed.source, &probed.sniffed, link).ok()?;
    Some(Preloaded { probed, doc })
}
