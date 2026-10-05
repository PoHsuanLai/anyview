//! Load's states, inputs and outputs.

use crate::stage::StageFamily;
use ds_core::word::Word;

pub use anyview_core::work::Ticket;

/// Whether the cheap first frame has arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeekFrame {
    /// Not yet; nothing shows.
    Pending,
    /// Shown while the full open continues.
    Ready,
}

/// Why a file did not open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum LoadFailure {
    /// The file is gone.
    NotFound,
    /// The file cannot be read (permissions, I/O).
    Unreadable,
    /// The viewer has no way to show this kind of file.
    Unsupported,
    /// The file is valid but over what the viewer opens (a JSON document or a workbook past its
    /// budget).
    TooLarge,
    /// The file needs a password.
    Locked,
    /// The file is damaged or truncated.
    Damaged,
}

/// How a probed file is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadFlow {
    /// A cheap first frame (a thumbnail, the first page, the first lines) while the full open
    /// runs beside it.
    PeekThenOpen,
    /// No cheap frame exists (media): only the full open runs.
    OpenOnly,
}

/// Where a load is. Every state names the load it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Nothing has been asked for yet; `ticket` is the last one issued.
    Idle { ticket: Ticket },
    /// Finding out what the file is.
    Probing { ticket: Ticket },
    /// Kind known; the peek and the full open run side by side.
    Peeking { ticket: Ticket, frame: PeekFrame },
    /// Kind known, no peek exists; the full open runs.
    Opening { ticket: Ticket },
    /// The full open is showing.
    Ready { ticket: Ticket },
    /// The file did not open.
    Failed { ticket: Ticket, reason: LoadFailure },
}

impl Default for Load {
    fn default() -> Self {
        Load::Idle {
            ticket: Ticket::default(),
        }
    }
}

/// What moves a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadIn {
    /// Open a file: any load in flight is abandoned.
    Begin,
    /// The probe of `ticket` named the file's stage and how to open it.
    Probed {
        ticket: Ticket,
        flow: LoadFlow,
        stage: StageFamily,
    },
    /// The cheap first frame of `ticket` is ready.
    Peeked { ticket: Ticket },
    /// The peek of `ticket` could not be made; the full open carries on.
    PeekFailed { ticket: Ticket },
    /// The full open of `ticket` is ready.
    Opened { ticket: Ticket },
    /// The probe or the open of `ticket` failed.
    Failed { ticket: Ticket, reason: LoadFailure },
    /// The clock; a load keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for LoadIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        LoadIn::Elapsed
    }
}

/// What a load wants done. Workers are told only the ticket; the one that carries them out keeps
/// the file the ticket was issued for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadOut {
    /// Sniff the file of this load.
    Probe(Ticket),
    /// Make the cheap first frame.
    Peek(Ticket),
    /// Open the file fully.
    Open(Ticket),
    /// Stop work for a load that was abandoned.
    Cancel(Ticket),
    /// Install a stage of this family for the file.
    UseStage(StageFamily),
    /// Paint the cheap first frame of this load.
    ShowFirstFrame(Ticket),
    /// Replace whatever shows with the full open of this load.
    ShowFull(Ticket),
}
