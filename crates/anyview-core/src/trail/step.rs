//! The trail's transitions.

use super::model::{Trail, TrailIn, TrailOut, TrailStacks};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step<V> = (Trail<V>, Vec<TrailOut<V>>);

impl<V: Clone + PartialEq + 'static> Trail<V> {
    /// The trail after `input`, and what it wants done: a step for a caller that keeps no clock
    /// (the trail has no timer).
    pub fn after(self, input: TrailIn<V>) -> (Self, Vec<TrailOut<V>>) {
        self.step(input, Stamp(0), &(), &())
    }
}

impl<V: Clone + PartialEq + 'static> Machine for Trail<V> {
    type In = TrailIn<V>;
    type Out = TrailOut<V>;
    type Params = ();
    type Ctx = ();

    fn step(self, input: TrailIn<V>, _at: Stamp, _params: &(), _cx: &()) -> Step<V> {
        match self {
            Trail::Resting(stacks) => resting(stacks, input),
            Trail::Saving(stacks) => saving(stacks, input),
            Trail::Undoing { stacks, taken } => undoing(stacks, taken, input),
            Trail::Redoing { stacks, taken } => redoing(stacks, taken, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Trail::Resting(_)
            | Trail::Saving(_)
            | Trail::Undoing { .. }
            | Trail::Redoing { .. } => None,
        }
    }
}

fn resting<V: Clone>(mut stacks: TrailStacks<V>, input: TrailIn<V>) -> Step<V> {
    match input {
        TrailIn::Save => (Trail::Saving(stacks), vec![TrailOut::Save]),
        TrailIn::Undo => match stacks.done.pop() {
            Some(taken) => {
                let out = TrailOut::Restore(taken.clone());
                (Trail::Undoing { stacks, taken }, vec![out])
            }
            None => (Trail::Resting(stacks), vec![TrailOut::NothingToUndo]),
        },
        TrailIn::Redo => match stacks.undone.pop() {
            Some(taken) => {
                let out = TrailOut::Restore(taken.clone());
                (Trail::Redoing { stacks, taken }, vec![out])
            }
            None => (Trail::Resting(stacks), vec![TrailOut::NothingToRedo]),
        },
        TrailIn::Kept(_) | TrailIn::Failed | TrailIn::Elapsed => (Trail::Resting(stacks), vec![]),
    }
}

/// A save keeps what it replaced, which undo goes back to; it makes every
/// earlier undo a branch that is gone.
fn saving<V>(mut stacks: TrailStacks<V>, input: TrailIn<V>) -> Step<V> {
    match input {
        TrailIn::Kept(version) => {
            stacks.done.push(version);
            stacks.undone.clear();
            (Trail::Resting(stacks), vec![])
        }
        TrailIn::Failed => (Trail::Resting(stacks), vec![]),
        TrailIn::Save | TrailIn::Undo | TrailIn::Redo => {
            (Trail::Saving(stacks), vec![TrailOut::Busy])
        }
        TrailIn::Elapsed => (Trail::Saving(stacks), vec![]),
    }
}

/// An undo keeps the file it replaced, which redo goes forward to; a failed one leaves the
/// version it was putting back where it was.
fn undoing<V>(mut stacks: TrailStacks<V>, taken: V, input: TrailIn<V>) -> Step<V> {
    match input {
        TrailIn::Kept(version) => {
            stacks.undone.push(version);
            (Trail::Resting(stacks), vec![])
        }
        TrailIn::Failed => {
            stacks.done.push(taken);
            (Trail::Resting(stacks), vec![])
        }
        TrailIn::Save | TrailIn::Undo | TrailIn::Redo => {
            (Trail::Undoing { stacks, taken }, vec![TrailOut::Busy])
        }
        TrailIn::Elapsed => (Trail::Undoing { stacks, taken }, vec![]),
    }
}

/// A redo keeps the file it replaced, which undo goes back to again.
fn redoing<V>(mut stacks: TrailStacks<V>, taken: V, input: TrailIn<V>) -> Step<V> {
    match input {
        TrailIn::Kept(version) => {
            stacks.done.push(version);
            (Trail::Resting(stacks), vec![])
        }
        TrailIn::Failed => {
            stacks.undone.push(taken);
            (Trail::Resting(stacks), vec![])
        }
        TrailIn::Save | TrailIn::Undo | TrailIn::Redo => {
            (Trail::Redoing { stacks, taken }, vec![TrailOut::Busy])
        }
        TrailIn::Elapsed => (Trail::Redoing { stacks, taken }, vec![]),
    }
}
