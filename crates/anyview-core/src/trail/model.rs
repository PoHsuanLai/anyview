//! The trail's states, inputs and outputs.

use ds_core::machine::Elapsed;

/// The versions to go back to and the ones undone, each newest last. `done` holds the version
/// every save kept; `undone` the version every undo kept of the file it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailStacks<V> {
    /// What undo goes back to.
    pub done: Vec<V>,
    /// What redo goes forward to.
    pub undone: Vec<V>,
}

impl<V> Default for TrailStacks<V> {
    fn default() -> Self {
        TrailStacks {
            done: Vec::new(),
            undone: Vec::new(),
        }
    }
}

/// Where a file's undo history stands. One write is in flight at a time, so two quick requests
/// can never race on the file: the second finds the trail busy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trail<V> {
    /// Nothing is being written.
    Resting(TrailStacks<V>),
    /// A save, or a revert, is being written; it will keep the version of what it replaces.
    Saving(TrailStacks<V>),
    /// `taken`, the newest of `done`, is being put back.
    Undoing { stacks: TrailStacks<V>, taken: V },
    /// `taken`, the newest of `undone`, is being put back.
    Redoing { stacks: TrailStacks<V>, taken: V },
}

impl<V> Default for Trail<V> {
    fn default() -> Self {
        Trail::Resting(TrailStacks::default())
    }
}

/// What moves the trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrailIn<V> {
    /// An edit is wanted.
    Edit,
    /// This kept version is wanted back as the file.
    Revert(V),
    /// Go back one save.
    Undo,
    /// Go forward one undo.
    Redo,
    /// The write in flight ended, and kept this version of what it replaced.
    Kept(V),
    /// The write in flight ended without writing.
    Failed,
    /// The clock; the trail keeps no timer.
    Elapsed,
}

impl<V> From<Elapsed> for TrailIn<V> {
    fn from(_: Elapsed) -> Self {
        TrailIn::Elapsed
    }
}

/// What the trail wants done, or why it declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrailOut<V> {
    /// Write the edit.
    Save,
    /// Put this kept version back as the file.
    Restore(V),
    /// There is no save to go back through.
    NothingToUndo,
    /// There is no undo to go forward through.
    NothingToRedo,
    /// A write is in flight; the request is dropped.
    Busy,
}
