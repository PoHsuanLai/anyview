//! Load's transitions. A result is accepted only when its ticket is the current one.

use super::model::{Load, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Load, Vec<LoadOut>);

impl Machine for Load {
    type In = LoadIn;
    type Out = LoadOut;
    type Params = ();

    fn step(self, input: LoadIn, _at: Stamp, _params: &()) -> Step {
        match self {
            Load::Idle { ticket } | Load::Ready { ticket } => settled(self, ticket, input),
            Load::Failed { ticket, reason: _ } => settled(self, ticket, input),
            Load::Probing { ticket } => probing(self, ticket, input),
            Load::Peeking { ticket, frame } => peeking(self, ticket, frame, input),
            Load::Opening { ticket } => opening(self, ticket, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Load::Idle { ticket: _ }
            | Load::Probing { ticket: _ }
            | Load::Peeking {
                ticket: _,
                frame: _,
            }
            | Load::Opening { ticket: _ }
            | Load::Ready { ticket: _ }
            | Load::Failed {
                ticket: _,
                reason: _,
            } => None,
        }
    }
}

/// A new load after `ticket`; `abandoned` is the one it replaces when that was still working.
fn begin(ticket: Ticket, abandoned: Option<Ticket>) -> Step {
    let next = ticket.next();
    let outs = abandoned
        .map(LoadOut::Cancel)
        .into_iter()
        .chain([LoadOut::Probe(next)])
        .collect();
    (Load::Probing { ticket: next }, outs)
}

/// Idle, ready and failed: nothing is in flight, so only `Begin` does anything, and every result
/// that arrives is late.
fn settled(this: Load, ticket: Ticket, input: LoadIn) -> Step {
    match input {
        LoadIn::Begin => begin(ticket, None),
        LoadIn::Probed { .. }
        | LoadIn::Peeked { .. }
        | LoadIn::PeekFailed { .. }
        | LoadIn::Opened { .. }
        | LoadIn::Failed { .. }
        | LoadIn::Elapsed => (this, vec![]),
    }
}

fn probing(this: Load, ticket: Ticket, input: LoadIn) -> Step {
    match input {
        LoadIn::Begin => begin(ticket, Some(ticket)),
        LoadIn::Probed {
            ticket: from,
            flow,
            stage,
        } if from == ticket => match flow {
            LoadFlow::PeekThenOpen => {
                let state = Load::Peeking {
                    ticket,
                    frame: PeekFrame::Pending,
                };
                let outs = vec![
                    LoadOut::UseStage(stage),
                    LoadOut::Peek(ticket),
                    LoadOut::Open(ticket),
                ];
                (state, outs)
            }
            LoadFlow::OpenOnly => (
                Load::Opening { ticket },
                vec![LoadOut::UseStage(stage), LoadOut::Open(ticket)],
            ),
        },
        LoadIn::Failed {
            ticket: from,
            reason,
        } if from == ticket => (Load::Failed { ticket, reason }, vec![]),
        LoadIn::Probed { .. }
        | LoadIn::Peeked { .. }
        | LoadIn::PeekFailed { .. }
        | LoadIn::Opened { .. }
        | LoadIn::Failed { .. }
        | LoadIn::Elapsed => (this, vec![]),
    }
}

fn peeking(this: Load, ticket: Ticket, frame: PeekFrame, input: LoadIn) -> Step {
    match input {
        LoadIn::Begin => begin(ticket, Some(ticket)),
        LoadIn::Peeked { ticket: from } if from == ticket => match frame {
            PeekFrame::Pending => (
                Load::Peeking {
                    ticket,
                    frame: PeekFrame::Ready,
                },
                vec![LoadOut::ShowFirstFrame(ticket)],
            ),
            PeekFrame::Ready => (this, vec![]),
        },
        LoadIn::Opened { ticket: from } if from == ticket => {
            (Load::Ready { ticket }, vec![LoadOut::ShowFull(ticket)])
        }
        LoadIn::Failed {
            ticket: from,
            reason,
        } if from == ticket => (Load::Failed { ticket, reason }, vec![]),
        LoadIn::Probed { .. }
        | LoadIn::Peeked { .. }
        | LoadIn::PeekFailed { .. }
        | LoadIn::Opened { .. }
        | LoadIn::Failed { .. }
        | LoadIn::Elapsed => (this, vec![]),
    }
}

fn opening(this: Load, ticket: Ticket, input: LoadIn) -> Step {
    match input {
        LoadIn::Begin => begin(ticket, Some(ticket)),
        LoadIn::Opened { ticket: from } if from == ticket => {
            (Load::Ready { ticket }, vec![LoadOut::ShowFull(ticket)])
        }
        LoadIn::Failed {
            ticket: from,
            reason,
        } if from == ticket => (Load::Failed { ticket, reason }, vec![]),
        LoadIn::Probed { .. }
        | LoadIn::Peeked { .. }
        | LoadIn::PeekFailed { .. }
        | LoadIn::Opened { .. }
        | LoadIn::Failed { .. }
        | LoadIn::Elapsed => (this, vec![]),
    }
}
