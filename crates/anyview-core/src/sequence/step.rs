//! Walking a sequence: the pure moves and the files worth preloading.

use super::Sequence;
use super::model::SequencePosition;
use crate::source::FilePath;

/// A request to move within a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceMove {
    /// → : the next entry.
    Next,
    /// ← : the previous entry.
    Previous,
    /// Home: the first entry.
    First,
    /// End: the last entry.
    Last,
    /// A given entry.
    To(SequencePosition),
}

/// `seq` after `mv`. A sequence stops at its ends: `Next` on the last entry and `Previous` on the
/// first leave it where it is, as the Finder does. `To` a position past the end goes to the last
/// entry.
#[must_use]
pub fn moved(seq: Sequence, mv: SequenceMove) -> Sequence {
    let here = seq.at().index();
    let target = match mv {
        SequenceMove::Next => here.saturating_add(1),
        SequenceMove::Previous => here.saturating_sub(1),
        SequenceMove::First => 0,
        SequenceMove::Last => usize::MAX,
        SequenceMove::To(position) => position.index(),
    };
    seq.pointing_at(target)
}

/// Which way a walk was last going, so a file that has gone is stepped over the way the person
/// was heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    /// Towards the end of the list.
    Onward,
    /// Towards the start of the list.
    Back,
}

/// `seq` without the file it is on, now on the file that follows it (the one before when it was
/// last); with `Heading::Back`, on the one before it (the one after when it was first). `None`
/// when it was the only file.
#[must_use]
pub fn without_current(seq: Sequence, heading: Heading) -> Option<Sequence> {
    let here = seq.at().index();
    let kept: Vec<FilePath> = seq
        .entries()
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != here)
        .map(|(_, entry)| entry.clone())
        .collect();
    let entries = super::NonEmpty::from_vec(kept)?;
    let target = match heading {
        Heading::Onward => here,
        Heading::Back => here.saturating_sub(1),
    };
    let rest = Sequence::new(entries, seq.origin().clone());
    let at = rest.position_at(target);
    Some(moved(rest, SequenceMove::To(at)))
}

/// The entries on either side of the current one: what to preload so the next arrow press is
/// instant. An end has no neighbour on that side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbours {
    /// The entry before the current one.
    pub previous: Option<FilePath>,
    /// The entry after the current one.
    pub next: Option<FilePath>,
}

/// The entries next to where `seq` points.
#[must_use]
pub fn neighbours(seq: &Sequence) -> Neighbours {
    let here = seq.at().index();
    let entry = |index: usize| seq.entries().get(index).cloned();
    Neighbours {
        previous: here.checked_sub(1).and_then(entry),
        next: here.checked_add(1).and_then(entry),
    }
}
