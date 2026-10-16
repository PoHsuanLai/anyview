//! Running a job on a worker: each kind of job blocks on the disk, a decoder or a highlighter.

use super::super::error::OpenError;
use super::super::folder::folder_sequence;
use super::super::seams::{FileLocks, ResumeSource, VersionSource};
use super::open::{Keeping, OpenPort, Opened, Preloaded, Replaces, WorkLane};
use super::vocab::{Done, Job};
use crate::Ticket;
use crate::families::{open_for, peek_for, search_sheet, search_tree, sort_sheet};
use anyview_core::FilePath;
use anyview_core::work::Stop;
use anyview_text::EditRefusal;

/// The largest file a preload opens: the next file is worth opening ahead only while that is
/// cheap, and a file this large is better opened when it is asked for.
const PRELOAD_LIMIT: u64 = 64 * 1024 * 1024;

impl Job {
    /// How soon the job is wanted: a pool that has a queue for each runs the nearest first.
    #[must_use]
    pub fn lane(&self) -> WorkLane {
        match self {
            Job::Probe { .. }
            | Job::Peek { .. }
            | Job::Open { .. }
            | Job::Lines { .. }
            | Job::Search { .. }
            | Job::SearchCells { .. }
            | Job::SearchNodes { .. }
            | Job::Sort { .. }
            | Job::EditRead { .. }
            | Job::DataBytes { .. }
            | Job::Stat { .. }
            | Job::Folder { .. }
            | Job::Versions { .. } => WorkLane::Visible,
            Job::Preload { .. } | Job::Keep { .. } => WorkLane::Preload,
            Job::Pdf(task) => task.lane(),
        }
    }

    /// Which older job of its load a newer one of this kind makes pointless: a search is replaced
    /// by the next query, a window of lines by the next window scrolled to.
    #[must_use]
    pub(in crate::io) fn replaces(&self) -> Replaces {
        if matches!(
            self,
            Job::Search { .. } | Job::SearchCells { .. } | Job::SearchNodes { .. }
        ) {
            Replaces::Search
        } else if matches!(self, Job::Lines { .. }) {
            Replaces::Lines
        } else {
            Replaces::Nothing
        }
    }

    /// The load this job belongs to; `Ticket::default()` for a job that belongs to none.
    #[must_use]
    pub fn ticket(&self) -> Ticket {
        match self {
            Job::Probe { ticket, .. }
            | Job::Peek { ticket, .. }
            | Job::Open { ticket, .. }
            | Job::Lines { ticket, .. }
            | Job::Search { ticket, .. }
            | Job::SearchCells { ticket, .. }
            | Job::SearchNodes { ticket, .. }
            | Job::Sort { ticket, .. }
            | Job::DataBytes { ticket, .. }
            | Job::EditRead { ticket, .. } => *ticket,
            Job::Pdf(task) => task.ticket(),
            Job::Preload { .. }
            | Job::Stat { .. }
            | Job::Folder { .. }
            | Job::Versions { .. }
            | Job::Keep { .. } => Ticket::default(),
        }
    }

    /// What a job that panicked answers, so the window that waits for it hears of the failure: the
    /// same message its own error would have been.
    pub(in crate::io) fn crashed(&self) -> Done {
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
            Job::Search {
                ticket,
                query,
                options,
                ..
            }
            | Job::SearchCells {
                ticket,
                query,
                options,
                ..
            }
            | Job::SearchNodes {
                ticket,
                query,
                options,
                ..
            } => Done::Found {
                ticket: *ticket,
                query: query.clone(),
                options: *options,
                result: Err(OpenError::Crashed),
            },
            Job::Sort { ticket, .. } => Done::Sorted {
                ticket: *ticket,
                order: None,
            },
            Job::EditRead { ticket, .. } => Done::EditText {
                ticket: *ticket,
                result: Err(EditRefusal::Unreadable(std::io::ErrorKind::Other)),
            },
            Job::DataBytes { ticket, stamp, .. } => Done::DataBytes {
                ticket: *ticket,
                stamp: *stamp,
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
            Job::Keep { .. } => Done::Kept,
        }
    }

    /// Do the work, blocking until it is done. `resume` is where the host keeps view memory.
    pub(in crate::io) fn run(
        self,
        resume: &dyn ResumeSource,
        locks: &dyn FileLocks,
        versions: &dyn VersionSource,
        stop: &Stop,
    ) -> Done {
        match self {
            Job::Probe { ticket, path } => Done::Probed {
                ticket,
                result: probed(&path, resume, locks),
            },
            Job::Peek {
                ticket,
                probed,
                link,
            } => Done::Peeked {
                ticket,
                result: peek_for(ticket, &probed.source, &probed.sniffed, &link, stop),
            },
            Job::Open {
                ticket,
                probed,
                link,
            } => Done::Opened {
                ticket,
                result: open_for(ticket, &probed.source, &probed.sniffed, &link, stop),
            },
            Job::Lines {
                ticket,
                doc,
                first,
                rows,
            } => Done::Lines {
                ticket,
                result: doc.window(first, rows, stop).map_err(OpenError::from),
            },
            Job::Search {
                ticket,
                doc,
                query,
                options,
                view,
            } => Done::Found {
                ticket,
                result: doc
                    .find(&query, options, view, stop)
                    .map_err(OpenError::from),
                query,
                options,
            },
            Job::SearchCells {
                ticket,
                doc,
                sheet,
                query,
                options,
            } => Done::Found {
                ticket,
                result: Ok(search_sheet(&doc, sheet, &query, options, stop)),
                query,
                options,
            },
            Job::SearchNodes {
                ticket,
                doc,
                query,
                options,
            } => Done::Found {
                ticket,
                result: Ok(search_tree(&doc, &query, options, stop)),
                query,
                options,
            },
            Job::Sort {
                ticket,
                doc,
                sheet,
                sort,
            } => Done::Sorted {
                ticket,
                order: sort_sheet(&doc, sheet, sort, stop),
            },
            Job::EditRead { ticket, doc } => Done::EditText {
                ticket,
                result: doc.read_for_edit(),
            },
            Job::DataBytes {
                ticket,
                doc,
                edits,
                stamp,
            } => Done::DataBytes {
                ticket,
                stamp,
                result: doc.written(&edits).map_err(OpenError::from),
            },
            Job::Preload { path, link } => Done::Preloaded {
                loaded: preloaded(&path, &link, resume, locks, stop),
                path,
            },
            Job::Stat { path } => Done::Stamped {
                stamp: super::super::probe::stamp_of(&path),
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
            Job::Keep { keeper, keeping } => {
                match keeping {
                    Keeping::Viewed(opened) => keeper.viewed(&opened),
                    Keeping::Place { source, resume } => keeper.remember(&source, &resume),
                }
                Done::Kept
            }
        }
    }
}

/// The probe of `path`, with what the host remembers of it and whether it can be written.
fn probed(
    path: &FilePath,
    resume: &dyn ResumeSource,
    locks: &dyn FileLocks,
) -> Result<Opened, OpenError> {
    let probed = super::super::probe(path)?;
    let remembered = resume.recall(path, probed.source.stamp());
    Ok(Opened {
        resume: remembered,
        access: locks.access(path),
        ..probed
    })
}

/// `path` opened, or `None` when it is too large, cannot be opened or opens to nothing the
/// person will not meet again by asking: a failed preload is silent, the open when asked reports.
fn preloaded(
    path: &FilePath,
    link: &OpenPort,
    resume: &dyn ResumeSource,
    locks: &dyn FileLocks,
    stop: &Stop,
) -> Option<Preloaded> {
    let probed = probed(path, resume, locks).ok()?;
    if probed.source.stamp().len.0 > PRELOAD_LIMIT {
        return None;
    }
    let doc = open_for(
        Ticket::default(),
        &probed.source,
        &probed.sniffed,
        link,
        stop,
    )
    .ok()?;
    Some(Preloaded { probed, doc })
}
