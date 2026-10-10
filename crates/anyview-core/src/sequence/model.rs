//! The sequence: the list that ← and → walk through, and where it came from.

use super::NonEmpty;
use crate::error::CoreError;
use crate::source::FilePath;

/// A place in a [`Sequence`]. Only a sequence makes one (`Sequence::position_at` and the
/// accessors), and every move clamps it to its own sequence, so a position can never address an
/// entry that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SequencePosition(usize);

impl SequencePosition {
    /// The entry's index, counting from zero.
    pub fn index(self) -> usize {
        self.0
    }
}

/// Which search produced a sequence of results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResultsId(pub u64);

/// Where a sequence came from, which says what its neighbours mean.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SequenceOrigin {
    /// The files of one folder.
    Folder(FilePath),
    /// The results of one search.
    Results(ResultsId),
    /// Files the person selected.
    Selection,
}

/// The files the viewer walks through with ← and →: never empty, always pointing at one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    entries: NonEmpty<FilePath>,
    at: SequencePosition,
    origin: SequenceOrigin,
}

impl Sequence {
    /// A sequence of `entries` pointing at the first.
    #[must_use]
    pub fn new(entries: NonEmpty<FilePath>, origin: SequenceOrigin) -> Self {
        Sequence {
            entries,
            at: SequencePosition(0),
            origin,
        }
    }

    /// A sequence of `entries` pointing at the first one equal to `current`, or why it is not
    /// among them.
    pub fn starting_at(
        entries: NonEmpty<FilePath>,
        current: &FilePath,
        origin: SequenceOrigin,
    ) -> Result<Self, CoreError> {
        let index = entries
            .iter()
            .position(|entry| entry == current)
            .ok_or_else(|| CoreError::NotInSequence {
                path: current.as_path().to_path_buf(),
            })?;
        Ok(Sequence {
            entries,
            at: SequencePosition(index),
            origin,
        })
    }

    /// Every entry, in order.
    pub fn entries(&self) -> &NonEmpty<FilePath> {
        &self.entries
    }

    /// Where the sequence points now.
    pub fn at(&self) -> SequencePosition {
        self.at
    }

    /// Where the sequence came from.
    pub fn origin(&self) -> &SequenceOrigin {
        &self.origin
    }

    /// The file the sequence points at.
    pub fn current(&self) -> &FilePath {
        self.entry(self.at)
    }

    /// The entry at `position`, which is in range because only a sequence makes positions and
    /// `position_at` clamps; a position from a longer sequence is read as its clamped self.
    pub fn entry(&self, position: SequencePosition) -> &FilePath {
        self.entries
            .get(position.0)
            .unwrap_or_else(|| self.entries.last())
    }

    /// The position of the entry at `index`, or of the last entry when `index` is past the end.
    pub fn position_at(&self, index: usize) -> SequencePosition {
        SequencePosition(index.min(self.entries.count().get() - 1)) // a count is at least 1
    }

    /// The same sequence pointing at `index`, clamped to the last entry.
    pub(super) fn pointing_at(self, index: usize) -> Sequence {
        let at = self.position_at(index);
        Sequence { at, ..self }
    }
}
