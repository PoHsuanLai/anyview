//! The history of the edits made to a table or a tree: the steps done, the steps taken back, and
//! the stamp of the state they leave, so "has unsaved changes" is a comparison. The text buffer keeps
//! its own history; this one is for the data views, whose steps are not text.

/// The state of the edits at a moment. Two equal stamps are the same edits; saving marks the stamp
/// that was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EditStamp(u64);

impl EditStamp {
    /// The stamp as a number, for a window that wants to tell a changed state from the last.
    #[must_use]
    pub fn number(self) -> u64 {
        self.0
    }
}

/// Steps done and steps taken back.
#[derive(Debug, Clone)]
pub(crate) struct Log<S> {
    done: Vec<(u64, S)>,
    undone: Vec<(u64, S)>,
    next: u64,
    clean: u64,
}

impl<S> Default for Log<S> {
    fn default() -> Self {
        Log {
            done: Vec::new(),
            undone: Vec::new(),
            next: 1,
            clean: 0,
        }
    }
}

impl<S> Log<S> {
    /// `step` has been done: what was taken back can no longer be done again.
    pub(crate) fn push(&mut self, step: S) {
        self.undone.clear();
        self.done.push((self.next, step));
        self.next += 1;
    }

    /// The last step done, which is now taken back; the caller reverses it.
    pub(crate) fn undo(&mut self) -> Option<&S> {
        let entry = self.done.pop()?;
        self.undone.push(entry);
        self.undone.last().map(|(_, step)| step)
    }

    /// The last step taken back, which is now done again; the caller applies it.
    pub(crate) fn redo(&mut self) -> Option<&S> {
        let entry = self.undone.pop()?;
        self.done.push(entry);
        self.done.last().map(|(_, step)| step)
    }

    /// How many steps can be taken back.
    pub(crate) fn undoable(&self) -> usize {
        self.done.len()
    }

    /// How many steps can be done again.
    pub(crate) fn redoable(&self) -> usize {
        self.undone.len()
    }

    /// The stamp of the state the steps leave.
    pub(crate) fn stamp(&self) -> EditStamp {
        EditStamp(self.done.last().map_or(0, |(id, _)| *id))
    }

    /// Whether the state differs from the one last marked saved (at first, the original).
    pub(crate) fn is_modified(&self) -> bool {
        self.stamp().0 != self.clean
    }

    /// The state of `stamp` is on disk.
    pub(crate) fn mark_saved(&mut self, stamp: EditStamp) {
        self.clean = stamp.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_is_modified_until_it_is_the_one_saved() {
        let mut log: Log<&str> = Log::default();
        assert!(!log.is_modified());
        log.push("a");
        log.push("b");
        assert!(log.is_modified());
        let saved = log.stamp();
        log.mark_saved(saved);
        assert!(!log.is_modified());
        assert_eq!(log.undo(), Some(&"b"));
        assert!(log.is_modified(), "back before the save");
        assert_eq!(log.redo(), Some(&"b"));
        assert!(!log.is_modified(), "and forward to it again");
        assert_eq!((log.undoable(), log.redoable()), (2, 0));
        log.undo();
        log.push("c");
        assert_eq!(
            (log.undoable(), log.redoable()),
            (2, 0),
            "a new step ends the redo"
        );
    }
}
