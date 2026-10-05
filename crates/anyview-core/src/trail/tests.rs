use super::*;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

/// A trail state written so a table can hold it: the versions are small numbers.
#[derive(Debug, Clone, Copy)]
enum Shape {
    Resting(&'static [u8], &'static [u8]),
    Saving(&'static [u8], &'static [u8]),
    Undoing(&'static [u8], &'static [u8], u8),
    Redoing(&'static [u8], &'static [u8], u8),
}

fn stacks(done: &[u8], undone: &[u8]) -> TrailStacks<u8> {
    TrailStacks {
        done: done.to_vec(),
        undone: undone.to_vec(),
    }
}

fn built(shape: Shape) -> Trail<u8> {
    match shape {
        Shape::Resting(done, undone) => Trail::Resting(stacks(done, undone)),
        Shape::Saving(done, undone) => Trail::Saving(stacks(done, undone)),
        Shape::Undoing(done, undone, taken) => Trail::Undoing {
            stacks: stacks(done, undone),
            taken,
        },
        Shape::Redoing(done, undone, taken) => Trail::Redoing {
            stacks: stacks(done, undone),
            taken,
        },
    }
}

use Shape::{Redoing, Resting, Saving, Undoing};
use TrailIn::{Edit, Failed, Kept, Redo, Revert, Undo};
use TrailOut::{Busy, NothingToRedo, NothingToUndo, Restore, Save};

type Case = (
    &'static str,
    Shape,
    TrailIn<u8>,
    Shape,
    &'static [TrailOut<u8>],
);

const CASES: &[Case] = &[
    // name, trail, input, trail after, outputs
    (
        "an edit asks for a save",
        Resting(&[], &[]),
        Edit,
        Saving(&[], &[]),
        &[Save],
    ),
    (
        "a save keeps the version of what it replaced",
        Saving(&[], &[]),
        Kept(1),
        Resting(&[1], &[]),
        &[],
    ),
    (
        "a save clears what could be redone",
        Saving(&[1], &[2, 3]),
        Kept(4),
        Resting(&[1, 4], &[]),
        &[],
    ),
    (
        "a failed save keeps nothing",
        Saving(&[1], &[2]),
        Failed,
        Resting(&[1], &[2]),
        &[],
    ),
    (
        "undo takes the newest save",
        Resting(&[1, 2], &[]),
        Undo,
        Undoing(&[1], &[], 2),
        &[Restore(2)],
    ),
    (
        "an undone file keeps the version it replaced for redo",
        Undoing(&[1], &[], 2),
        Kept(3),
        Resting(&[1], &[3]),
        &[],
    ),
    (
        "a failed undo puts its version back",
        Undoing(&[1], &[], 2),
        Failed,
        Resting(&[1, 2], &[]),
        &[],
    ),
    (
        "undo with nothing saved says so",
        Resting(&[], &[5]),
        Undo,
        Resting(&[], &[5]),
        &[NothingToUndo],
    ),
    (
        "redo takes the newest undo",
        Resting(&[1], &[3, 4]),
        Redo,
        Redoing(&[1], &[3], 4),
        &[Restore(4)],
    ),
    (
        "a redone file keeps the version it replaced for undo",
        Redoing(&[1], &[3], 4),
        Kept(6),
        Resting(&[1, 6], &[3]),
        &[],
    ),
    (
        "a failed redo puts its version back",
        Redoing(&[1], &[3], 4),
        Failed,
        Resting(&[1], &[3, 4]),
        &[],
    ),
    (
        "redo with nothing undone says so",
        Resting(&[1], &[]),
        Redo,
        Resting(&[1], &[]),
        &[NothingToRedo],
    ),
    (
        "a revert asks for that version",
        Resting(&[1], &[2]),
        Revert(9),
        Saving(&[1], &[2]),
        &[Restore(9)],
    ),
    (
        "a revert is undoable and ends the redo branch",
        Saving(&[1], &[2]),
        Kept(7),
        Resting(&[1, 7], &[]),
        &[],
    ),
    (
        "a save in flight refuses an edit",
        Saving(&[], &[]),
        Edit,
        Saving(&[], &[]),
        &[Busy],
    ),
    (
        "a save in flight refuses undo",
        Saving(&[1], &[]),
        Undo,
        Saving(&[1], &[]),
        &[Busy],
    ),
    (
        "an undo in flight refuses redo",
        Undoing(&[], &[], 1),
        Redo,
        Undoing(&[], &[], 1),
        &[Busy],
    ),
    (
        "a redo in flight refuses a revert",
        Redoing(&[], &[], 1),
        Revert(2),
        Redoing(&[], &[], 1),
        &[Busy],
    ),
    (
        "a write that nobody started changes nothing",
        Resting(&[1], &[]),
        Kept(2),
        Resting(&[1], &[]),
        &[],
    ),
];

#[test]
fn every_row_of_the_trail_table_steps_as_written() {
    for (name, before, input, after, outs) in CASES {
        let (next, out) = built(*before).step(input.clone(), Stamp(0), &());
        assert_eq!(next, built(*after), "{name}: state");
        assert_eq!(&out, outs, "{name}: outputs");
    }
}

#[test]
fn undo_then_redo_then_undo_walk_the_same_versions() {
    let step = |trail: Trail<u8>, input| trail.step(input, Stamp(0), &()).0;
    let trail = step(step(Trail::default(), Edit), Kept(1));
    let trail = step(step(trail, Undo), Kept(2));
    assert_eq!(trail, Trail::Resting(stacks(&[], &[2])), "undone");
    let trail = step(step(trail, Redo), Kept(3));
    assert_eq!(trail, Trail::Resting(stacks(&[3], &[])), "redone");
}
