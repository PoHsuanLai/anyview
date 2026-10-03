//! Navigation's transitions, over the core sequence's pure moves.

use super::model::{Navigate, NavigateIn, NavigateOut};
use anyview_core::{Sequence, SequenceMove, moved, neighbours};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Navigate, Vec<NavigateOut>);

impl Machine for Navigate {
    type In = NavigateIn;
    type Out = NavigateOut;
    type Params = ();

    fn step(self, input: NavigateIn, _at: Stamp, _params: &()) -> Step {
        match self {
            Navigate::Idle => idle(input),
            Navigate::Walking { sequence } => walking(sequence, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Navigate::Idle | Navigate::Walking { sequence: _ } => None,
        }
    }
}

fn idle(input: NavigateIn) -> Step {
    match input {
        NavigateIn::Start(sequence) => {
            let preload = NavigateOut::Preload(neighbours(&sequence));
            (Navigate::Walking { sequence }, vec![preload])
        }
        NavigateIn::Next
        | NavigateIn::Previous
        | NavigateIn::First
        | NavigateIn::Last
        | NavigateIn::Leave
        | NavigateIn::Elapsed => (Navigate::Idle, vec![]),
    }
}

fn walking(sequence: Sequence, input: NavigateIn) -> Step {
    match input {
        NavigateIn::Start(next) => {
            let preload = NavigateOut::Preload(neighbours(&next));
            (Navigate::Walking { sequence: next }, vec![preload])
        }
        NavigateIn::Next => walk(sequence, SequenceMove::Next),
        NavigateIn::Previous => walk(sequence, SequenceMove::Previous),
        NavigateIn::First => walk(sequence, SequenceMove::First),
        NavigateIn::Last => walk(sequence, SequenceMove::Last),
        NavigateIn::Leave => (Navigate::Idle, vec![]),
        NavigateIn::Elapsed => (Navigate::Walking { sequence }, vec![]),
    }
}

/// Moves; a move that lands where the walk already is (the end of the list) opens nothing.
fn walk(sequence: Sequence, movement: SequenceMove) -> Step {
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
    (Navigate::Walking { sequence: after }, outs)
}
