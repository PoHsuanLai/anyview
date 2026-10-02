//! The work a window hands its workers, and what comes back. Each job blocks (disk, decoding,
//! highlighting); none touches the UI thread's state. A job that shows pixels uploads them into
//! the window's texture itself, so a decoded picture never passes through the UI thread: its
//! result is a small document that names the texture.

use super::error::OpenError;
use crate::families::{LineWindow, LoadedDoc, PdfAnswer, PdfTask, TextDoc, open_for};
use crate::{StageFamily, Ticket};
use anyview_core::{FilePath, LineIndex, Sniffed, Source};
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use std::sync::Arc;

/// What an open needs from the window: the texture a picture is uploaded into (made on the UI
/// thread from the window's `Gpu`) and the one highlighter every file shares.
#[derive(Debug, Clone)]
pub struct OpenLink {
    /// Where a decoded picture goes.
    pub texture: TextureHandle,
    /// Highlights code for every stage.
    pub highlighter: Arc<Highlighter>,
}

/// A file whose type was read: what to open, how it was sniffed and which stage shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    /// The file and its stamp.
    pub source: Source,
    /// Proof the type was sniffed.
    pub sniffed: Sniffed,
    /// The stage that shows it.
    pub family: StageFamily,
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
    /// Draw tiles, a thumbnail or a search of an open PDF.
    Pdf(PdfTask),
}

/// What a worker made of a job.
#[derive(Debug)]
pub enum Done {
    /// The probe of `ticket`.
    Probed {
        ticket: Ticket,
        result: Result<Probed, OpenError>,
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
    /// What a PDF task made, for the load that asked.
    Pdf { ticket: Ticket, answer: PdfAnswer },
}

impl Job {
    /// How soon the job is wanted.
    pub fn lane(&self) -> WorkLane {
        match self {
            Job::Probe { .. } | Job::Open { .. } | Job::Lines { .. } => WorkLane::Visible,
            Job::Pdf(task) => task.lane(),
        }
    }

    /// Do the work, blocking until it is done.
    pub fn run(self) -> Done {
        match self {
            Job::Probe { ticket, path } => Done::Probed {
                ticket,
                result: super::probe(&path),
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
            Job::Pdf(task) => Done::Pdf {
                ticket: task.ticket(),
                answer: task.run(),
            },
        }
    }
}
