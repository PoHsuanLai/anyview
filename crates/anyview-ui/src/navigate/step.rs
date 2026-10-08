//! Navigation's transitions, over the core sequence's pure moves.

use super::model::{Navigate, NavigateIn, NavigateOut};
use anyview_core::{Heading, Sequence, SequenceMove, moved, neighbours, without_current};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Navigate, Vec<NavigateOut>);

impl Machine for Navigate {
    type In = NavigateIn;
    type Out = NavigateOut;
    type Params = ();
    type Ctx = ();

    fn step(self, input: NavigateIn, _at: Stamp, _params: &(), _cx: &()) -> Step {
        match self {
            Navigate::Idle => idle(input),
            Navigate::Walking { sequence, heading } => walking(sequence, heading, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Navigate::Idle | Navigate::Walking { .. } => None,
        }
    }
}

fn idle(input: NavigateIn) -> Step {
    match input {
        NavigateIn::Start(sequence) => {
            let preload = NavigateOut::Preload(neighbours(&sequence));
            (
                Navigate::Walking {
                    sequence,
                    heading: Heading::Onward,
                },
                vec![preload],
            )
        }
        NavigateIn::Next
        | NavigateIn::Previous
        | NavigateIn::First
        | NavigateIn::Last
        | NavigateIn::Leave
        | NavigateIn::Gone
        | NavigateIn::Elapsed => (Navigate::Idle, vec![]),
    }
}

fn walking(sequence: Sequence, heading: Heading, input: NavigateIn) -> Step {
    match input {
        NavigateIn::Start(next) => {
            let preload = NavigateOut::Preload(neighbours(&next));
            (
                Navigate::Walking {
                    sequence: next,
                    heading: Heading::Onward,
                },
                vec![preload],
            )
        }
        NavigateIn::Next => walk(sequence, SequenceMove::Next, Heading::Onward),
        NavigateIn::Previous => walk(sequence, SequenceMove::Previous, Heading::Back),
        NavigateIn::First => walk(sequence, SequenceMove::First, Heading::Onward),
        NavigateIn::Last => walk(sequence, SequenceMove::Last, Heading::Back),
        NavigateIn::Gone => gone(sequence, heading),
        NavigateIn::Leave => (Navigate::Idle, vec![]),
        NavigateIn::Elapsed => (Navigate::Walking { sequence, heading }, vec![]),
    }
}

/// Moves; a move that lands where the walk already is (the end of the list) opens nothing.
fn walk(sequence: Sequence, movement: SequenceMove, heading: Heading) -> Step {
    let before = sequence.at();
    let after = moved(sequence, movement);
    let outs = if after.at() == before {
        vec![]
    } else {
        vec![
            NavigateOut::Open(after.current().clone()),
            NavigateOut::Preload(neighbours(&after)),
        ]
    };
    (
        Navigate::Walking {
            sequence: after,
            heading,
        },
        outs,
    )
}

/// The file the walk is on is gone: it leaves the list and the walk opens the file the person
/// was heading for. The last file has nothing to move on to, so the walk stays on it.
fn gone(sequence: Sequence, heading: Heading) -> Step {
    match without_current(sequence.clone(), heading) {
        Some(rest) => {
            let outs = vec![
                NavigateOut::Open(rest.current().clone()),
                NavigateOut::Preload(neighbours(&rest)),
            ];
            (
                Navigate::Walking {
                    sequence: rest,
                    heading,
                },
                outs,
            )
        }
        None => (Navigate::Walking { sequence, heading }, vec![]),
    }
}
