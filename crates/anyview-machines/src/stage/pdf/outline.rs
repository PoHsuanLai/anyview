//! The PDF outline's folds: which bookmarks have their children folded away, and the bookmark
//! the cursor is on. The bookmarks come as a list in reading order with a depth each, so a
//! bookmark is a branch when the next one sits deeper, and its children are the run of deeper
//! bookmarks that follows it. The arrows move the cursor along the bookmarks in view; Right
//! unfolds a folded branch and then steps into it, Left folds an open branch and then steps out
//! to its parent, as Finder's and Preview's outlines do.

use crate::stage::row::{RowNo, RowStep};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use std::collections::BTreeSet;

/// How deep each bookmark sits, in reading order: what the machine needs of the document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutlineShape {
    depths: Vec<u32>,
}

impl OutlineShape {
    /// The shape of bookmarks at these depths (0 is the top), in reading order.
    #[must_use]
    pub fn of(depths: impl IntoIterator<Item = u32>) -> Self {
        OutlineShape {
            depths: depths.into_iter().collect(),
        }
    }

    fn depth(&self, at: u32) -> Option<u32> {
        self.depths.get(at as usize).copied()
    }

    /// Whether the bookmark `at` has children.
    #[must_use]
    pub fn is_branch(&self, at: u32) -> bool {
        match (self.depth(at), self.depth(at.saturating_add(1))) {
            (Some(here), Some(next)) => next > here,
            _ => false,
        }
    }

    /// The bookmark `at` sits under, when it is not at the top.
    fn parent(&self, at: u32) -> Option<u32> {
        let depth = self.depth(at)?;
        (0..at)
            .rev()
            .find(|before| self.depth(*before).is_some_and(|d| d < depth))
    }

    /// Whether `at` is one of the children, at any depth, of the bookmark `branch`.
    fn is_under(&self, branch: u32, at: u32) -> bool {
        let Some(top) = self.depth(branch) else {
            return false;
        };
        at > branch && (branch + 1..=at).all(|between| self.depth(between).is_some_and(|d| d > top))
    }
}

/// Which bookmarks are folded and where the cursor is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutlineFold {
    closed: BTreeSet<u32>,
    cursor: Option<u32>,
}

impl OutlineFold {
    /// The outline as it first opens, with these bookmarks folded and no cursor.
    #[must_use]
    pub fn folded(closed: impl IntoIterator<Item = u32>) -> Self {
        OutlineFold {
            closed: closed.into_iter().collect(),
            cursor: None,
        }
    }

    /// Whether the children of the bookmark `at` are folded away.
    #[must_use]
    pub fn is_closed(&self, at: u32) -> bool {
        self.closed.contains(&at)
    }

    /// The bookmark under the cursor.
    #[must_use]
    pub fn cursor(&self) -> Option<u32> {
        self.cursor
    }

    /// The bookmarks in view, in reading order: all but the children of a folded branch.
    #[must_use]
    pub fn visible(&self, shape: &OutlineShape) -> Vec<u32> {
        let mut shown = Vec::with_capacity(shape.depths.len());
        let mut folded_at: Option<u32> = None;
        for (at, depth) in (0_u32..).zip(shape.depths.iter().copied()) {
            if let Some(limit) = folded_at {
                if depth > limit {
                    continue;
                }
                folded_at = None;
            }
            shown.push(at);
            if self.closed.contains(&at) && shape.is_branch(at) {
                folded_at = Some(depth);
            }
        }
        shown
    }

    /// Folds `at`; a cursor inside it comes back to it.
    fn close(&mut self, at: u32, shape: &OutlineShape) {
        self.closed.insert(at);
        if self.cursor.is_some_and(|cursor| shape.is_under(at, cursor)) {
            self.cursor = Some(at);
        }
    }
}

/// What moves the outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineIn {
    /// The triangle of this bookmark was pressed: fold it if it is open, unfold it if it is
    /// folded. A bookmark with no children is left alone.
    Toggle(u32),
    /// The cursor moved to this bookmark (a press on its row).
    Select(u32),
    /// Move the cursor along the bookmarks in view by a row: Up, Down, Top or Bottom.
    Move(RowStep),
    /// Left: fold the bookmark under the cursor if it is open, otherwise step out to its parent.
    Fold,
    /// Right: unfold the bookmark under the cursor if it is folded, otherwise step into it.
    Unfold,
    /// The clock; the outline keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for OutlineIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        OutlineIn::Elapsed
    }
}

/// What the outline asks of the window: nothing. Which bookmarks are folded is not kept for next
/// time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineOut {}

impl Machine for OutlineFold {
    type In = OutlineIn;
    type Out = OutlineOut;
    type Params = OutlineShape;
    type Ctx = ();

    fn step(
        mut self,
        input: OutlineIn,
        _at: Stamp,
        shape: &OutlineShape,
        _cx: &(),
    ) -> (Self, Vec<OutlineOut>) {
        match input {
            OutlineIn::Toggle(at) => {
                if shape.is_branch(at) && !self.closed.remove(&at) {
                    self.close(at, shape);
                }
            }
            OutlineIn::Select(at) => {
                if shape.depth(at).is_some() {
                    self.cursor = Some(at);
                }
            }
            OutlineIn::Move(step) => {
                let shown = self.visible(shape);
                let rows = u32::try_from(shown.len()).unwrap_or(u32::MAX);
                let from = self
                    .cursor
                    .and_then(|cursor| shown.iter().position(|at| *at == cursor))
                    .and_then(|place| u32::try_from(place).ok())
                    .map(RowNo);
                if let Some(to) = step.from(from, rows, 1) {
                    self.cursor = shown.get(to.0 as usize).copied();
                }
            }
            OutlineIn::Fold => {
                if let Some(at) = self.cursor {
                    if shape.is_branch(at) && !self.closed.contains(&at) {
                        self.close(at, shape);
                    } else if let Some(parent) = shape.parent(at) {
                        self.cursor = Some(parent);
                    }
                }
            }
            OutlineIn::Unfold => {
                if let Some(at) = self.cursor
                    && shape.is_branch(at)
                    && !self.closed.remove(&at)
                {
                    self.cursor = Some(at + 1);
                }
            }
            OutlineIn::Elapsed => {}
        }
        (self, Vec::new())
    }

    fn wake(&self) -> Option<Stamp> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chapter 1 (open) with sections 1.1 (with a subsection) and 1.2, Chapter 2 (folded) with a
    /// section, and an Appendix.
    fn shape() -> OutlineShape {
        OutlineShape::of([0, 1, 2, 1, 0, 1, 0])
    }

    fn at(cursor: u32, closed: &[u32]) -> OutlineFold {
        OutlineFold {
            closed: closed.iter().copied().collect(),
            cursor: Some(cursor),
        }
    }

    #[test]
    fn the_outline_steps_as_the_table_says() {
        let down = OutlineIn::Move(RowStep::Down);
        let up = OutlineIn::Move(RowStep::Up);
        // name, state before, input, state after
        let cases: Vec<(&str, OutlineFold, OutlineIn, OutlineFold)> = vec![
            (
                "the first move lands on the first bookmark",
                OutlineFold::folded([4]),
                down,
                at(0, &[4]),
            ),
            (
                "down steps over the children of a folded branch",
                at(3, &[4]),
                down,
                at(4, &[4]),
            ),
            (
                "down from a folded branch lands past its children",
                at(4, &[4]),
                down,
                at(6, &[4]),
            ),
            (
                "up lands on the folded branch",
                at(6, &[4]),
                up,
                at(4, &[4]),
            ),
            (
                "down stops at the last bookmark",
                at(6, &[]),
                down,
                at(6, &[]),
            ),
            (
                "the end goes to the last bookmark in view",
                at(0, &[]),
                OutlineIn::Move(RowStep::Bottom),
                at(6, &[]),
            ),
            (
                "pressing a triangle folds an open branch",
                at(0, &[]),
                OutlineIn::Toggle(1),
                at(0, &[1]),
            ),
            (
                "pressing a triangle unfolds a folded branch",
                at(0, &[1]),
                OutlineIn::Toggle(1),
                at(0, &[]),
            ),
            (
                "pressing the triangle of a leaf does nothing",
                at(0, &[]),
                OutlineIn::Toggle(2),
                at(0, &[]),
            ),
            (
                "folding a branch takes a cursor inside it back to the branch",
                at(2, &[]),
                OutlineIn::Toggle(0),
                at(0, &[0]),
            ),
            (
                "Left folds an open branch",
                at(1, &[]),
                OutlineIn::Fold,
                at(1, &[1]),
            ),
            (
                "Left on a folded branch steps out to its parent",
                at(1, &[1]),
                OutlineIn::Fold,
                at(0, &[1]),
            ),
            (
                "Left on a leaf steps out to its parent",
                at(2, &[]),
                OutlineIn::Fold,
                at(1, &[]),
            ),
            (
                "Left at the top of a leaf stays",
                at(6, &[]),
                OutlineIn::Fold,
                at(6, &[]),
            ),
            (
                "Right unfolds a folded branch",
                at(4, &[4]),
                OutlineIn::Unfold,
                at(4, &[]),
            ),
            (
                "Right on an open branch steps into it",
                at(0, &[]),
                OutlineIn::Unfold,
                at(1, &[]),
            ),
            (
                "Right on a leaf does nothing",
                at(2, &[]),
                OutlineIn::Unfold,
                at(2, &[]),
            ),
            (
                "Left with no cursor does nothing",
                OutlineFold::folded([4]),
                OutlineIn::Fold,
                OutlineFold::folded([4]),
            ),
            (
                "selecting a bookmark that is not there is refused",
                at(0, &[]),
                OutlineIn::Select(9),
                at(0, &[]),
            ),
            (
                "selecting a bookmark moves the cursor",
                at(0, &[]),
                OutlineIn::Select(5),
                at(5, &[]),
            ),
        ];
        for (name, before, input, after) in cases {
            let (next, outs) = before.step(input, Stamp(0), &shape(), &());
            assert_eq!(next, after, "{name}");
            assert!(outs.is_empty(), "{name}");
            assert_eq!(next.wake(), None, "{name}: no timer");
        }
    }

    #[test]
    fn the_children_of_a_folded_branch_leave_the_view() {
        let shape = shape();
        // name, folded, the bookmarks in view
        let cases: &[(&str, &[u32], &[u32])] = &[
            ("all open", &[], &[0, 1, 2, 3, 4, 5, 6]),
            ("a section folded", &[1], &[0, 1, 3, 4, 5, 6]),
            ("a chapter folded", &[0], &[0, 4, 5, 6]),
            ("a leaf cannot fold", &[2], &[0, 1, 2, 3, 4, 5, 6]),
            ("folds inside a fold are kept", &[0, 1], &[0, 4, 5, 6]),
            ("the second chapter folded", &[4], &[0, 1, 2, 3, 4, 6]),
        ];
        for (name, closed, want) in cases {
            let fold = OutlineFold::folded(closed.iter().copied());
            assert_eq!(&fold.visible(&shape), want, "{name}");
        }
    }
}
