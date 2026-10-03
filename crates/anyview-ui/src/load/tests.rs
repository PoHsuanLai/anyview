use super::*;
use crate::stage::StageFamily;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const T1: Ticket = Ticket(1);
const T2: Ticket = Ticket(2);
const T3: Ticket = Ticket(3);

const fn probing(ticket: Ticket) -> Load {
    Load::Probing { ticket }
}
const fn peeking(ticket: Ticket, frame: PeekFrame) -> Load {
    Load::Peeking { ticket, frame }
}
const fn probed(ticket: Ticket, flow: LoadFlow, stage: StageFamily) -> LoadIn {
    LoadIn::Probed {
        ticket,
        flow,
        stage,
    }
}

/// Name, state before, input, state after, outputs.
type Case = (&'static str, Load, LoadIn, Load, &'static [LoadOut]);

const CASES: &[Case] = &[
    (
        "the first begin issues ticket one and probes",
        Load::Idle { ticket: Ticket(0) },
        LoadIn::Begin,
        probing(T1),
        &[LoadOut::Probe(T1)],
    ),
    (
        "an image probe starts the peek and the full open together",
        probing(T1),
        probed(T1, LoadFlow::PeekThenOpen, StageFamily::Raster),
        peeking(T1, PeekFrame::Pending),
        &[
            LoadOut::UseStage(StageFamily::Raster),
            LoadOut::Peek(T1),
            LoadOut::Open(T1),
        ],
    ),
    (
        "a media probe has no peek and opens only",
        probing(T1),
        probed(T1, LoadFlow::OpenOnly, StageFamily::Media),
        Load::Opening { ticket: T1 },
        &[LoadOut::UseStage(StageFamily::Media), LoadOut::Open(T1)],
    ),
    (
        "a probe result for a file already left is ignored",
        probing(T2),
        probed(T1, LoadFlow::PeekThenOpen, StageFamily::Raster),
        probing(T2),
        &[],
    ),
    (
        "a failed probe fails the load",
        probing(T1),
        LoadIn::Failed {
            ticket: T1,
            reason: LoadFailure::Unsupported,
        },
        Load::Failed {
            ticket: T1,
            reason: LoadFailure::Unsupported,
        },
        &[],
    ),
    (
        "a stale failure does not fail the current load",
        probing(T2),
        LoadIn::Failed {
            ticket: T1,
            reason: LoadFailure::NotFound,
        },
        probing(T2),
        &[],
    ),
    (
        "begin while probing cancels the old load and starts the next",
        probing(T1),
        LoadIn::Begin,
        probing(T2),
        &[LoadOut::Cancel(T1), LoadOut::Probe(T2)],
    ),
    (
        "the peek shows the first frame",
        peeking(T1, PeekFrame::Pending),
        LoadIn::Peeked { ticket: T1 },
        peeking(T1, PeekFrame::Ready),
        &[LoadOut::ShowFirstFrame(T1)],
    ),
    (
        "a second peek does not repaint",
        peeking(T1, PeekFrame::Ready),
        LoadIn::Peeked { ticket: T1 },
        peeking(T1, PeekFrame::Ready),
        &[],
    ),
    (
        "a stale peek is ignored",
        peeking(T2, PeekFrame::Pending),
        LoadIn::Peeked { ticket: T1 },
        peeking(T2, PeekFrame::Pending),
        &[],
    ),
    (
        "a failed peek leaves the full open running",
        peeking(T1, PeekFrame::Pending),
        LoadIn::PeekFailed { ticket: T1 },
        peeking(T1, PeekFrame::Pending),
        &[],
    ),
    (
        "the full open replaces the first frame",
        peeking(T1, PeekFrame::Ready),
        LoadIn::Opened { ticket: T1 },
        Load::Ready { ticket: T1 },
        &[LoadOut::ShowFull(T1)],
    ),
    (
        "a full open that beats the peek goes straight to ready",
        peeking(T1, PeekFrame::Pending),
        LoadIn::Opened { ticket: T1 },
        Load::Ready { ticket: T1 },
        &[LoadOut::ShowFull(T1)],
    ),
    (
        "a stale full open is ignored",
        peeking(T2, PeekFrame::Ready),
        LoadIn::Opened { ticket: T1 },
        peeking(T2, PeekFrame::Ready),
        &[],
    ),
    (
        "a failed open after the first frame fails the load",
        peeking(T1, PeekFrame::Ready),
        LoadIn::Failed {
            ticket: T1,
            reason: LoadFailure::Damaged,
        },
        Load::Failed {
            ticket: T1,
            reason: LoadFailure::Damaged,
        },
        &[],
    ),
    (
        "begin while peeking cancels the old load",
        peeking(T1, PeekFrame::Ready),
        LoadIn::Begin,
        probing(T2),
        &[LoadOut::Cancel(T1), LoadOut::Probe(T2)],
    ),
    (
        "opening becomes ready when the open lands",
        Load::Opening { ticket: T1 },
        LoadIn::Opened { ticket: T1 },
        Load::Ready { ticket: T1 },
        &[LoadOut::ShowFull(T1)],
    ),
    (
        "opening ignores a peek",
        Load::Opening { ticket: T1 },
        LoadIn::Peeked { ticket: T1 },
        Load::Opening { ticket: T1 },
        &[],
    ),
    (
        "a failed open fails the load",
        Load::Opening { ticket: T1 },
        LoadIn::Failed {
            ticket: T1,
            reason: LoadFailure::Unreadable,
        },
        Load::Failed {
            ticket: T1,
            reason: LoadFailure::Unreadable,
        },
        &[],
    ),
    (
        "a late peek never replaces the full open",
        Load::Ready { ticket: T1 },
        LoadIn::Peeked { ticket: T1 },
        Load::Ready { ticket: T1 },
        &[],
    ),
    (
        "begin from ready starts the next load with nothing to cancel",
        Load::Ready { ticket: T1 },
        LoadIn::Begin,
        probing(T2),
        &[LoadOut::Probe(T2)],
    ),
    (
        "begin from failed retries under a new ticket",
        Load::Failed {
            ticket: T2,
            reason: LoadFailure::NotFound,
        },
        LoadIn::Begin,
        probing(T3),
        &[LoadOut::Probe(T3)],
    ),
    (
        "the clock changes nothing",
        peeking(T1, PeekFrame::Pending),
        LoadIn::Elapsed,
        peeking(T1, PeekFrame::Pending),
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, from, input, state, outs) in CASES {
        let (next, out) = from.step(*input, Stamp(0), &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn leaving_a_file_mid_load_drops_every_result_of_the_first() {
    let params = ();
    let mut state = Load::default();
    let mut step = |input: LoadIn| {
        let (next, outs) = state.step(input, Stamp(0), &params);
        state = next;
        outs
    };
    assert_eq!(step(LoadIn::Begin), vec![LoadOut::Probe(T1)]);
    step(probed(T1, LoadFlow::PeekThenOpen, StageFamily::Pdf));
    assert_eq!(
        step(LoadIn::Begin),
        vec![LoadOut::Cancel(T1), LoadOut::Probe(T2)]
    );
    // The workers of the first file finish anyway.
    assert_eq!(step(LoadIn::Peeked { ticket: T1 }), vec![]);
    assert_eq!(step(LoadIn::Opened { ticket: T1 }), vec![]);
    assert_eq!(state, probing(T2));
}
