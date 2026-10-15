//! The tree stage's states and inputs.

use crate::stage::data_edits::{DataEdits, EditSessionIn, EditSessionOut, TypingIn, TypingState};
use crate::stage::find::{FindOut, HitCount, HitIndex, StageFind};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::text::Editable;
use crate::stage::zoom::{ContentScale, ZoomDir};
use crate::typed::TypedText;
use anyview_core::{OpenNodes, TreePath};

/// What the tree stage is doing. The top level of a document is open when a file opens. A find
/// that is up has its hit under the cursor, with the nodes above it opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeStage {
    /// Reading, with no row picked.
    Browsing {
        /// The nodes that are open.
        open: OpenNodes,
        /// The find that is up, when one is.
        find: Option<StageFind>,
        /// How large the rows are drawn.
        scale: ContentScale,
        /// What is typed in, and what the window says of the edits.
        edits: DataEdits,
    },
    /// Reading with the cursor on a row.
    Selected {
        /// The nodes that are open.
        open: OpenNodes,
        /// The row under the cursor.
        row: RowNo,
        /// The find that is up, when one is.
        find: Option<StageFind>,
        /// How large the rows are drawn.
        scale: ContentScale,
        /// What is typed in, and what the window says of the edits.
        edits: DataEdits,
    },
}

impl TreeStage {
    /// The stage with the cursor on `row`, or on none.
    pub(super) fn of(open: OpenNodes, row: Option<RowNo>, look: Look) -> Self {
        let Look { find, scale, edits } = look;
        match row {
            Some(row) => TreeStage::Selected {
                open,
                row,
                find,
                scale,
                edits,
            },
            // With no row under the cursor nothing is typed in.
            None => TreeStage::Browsing {
                open,
                find,
                scale,
                edits: DataEdits {
                    typing: TypingState::Idle,
                    ..edits
                },
            },
        }
    }

    /// The open nodes, the row and the look, taken apart.
    pub(super) fn into_parts(self) -> (OpenNodes, Option<RowNo>, Look) {
        match self {
            TreeStage::Browsing {
                open,
                find,
                scale,
                edits,
            } => (open, None, Look { find, scale, edits }),
            TreeStage::Selected {
                open,
                row,
                find,
                scale,
                edits,
            } => (open, Some(row), Look { find, scale, edits }),
        }
    }

    /// What is typed in, and what the window says of the edits.
    #[must_use]
    pub fn edits(&self) -> &DataEdits {
        match self {
            TreeStage::Browsing { edits, .. } | TreeStage::Selected { edits, .. } => edits,
        }
    }

    /// The nodes that are open.
    #[must_use]
    pub fn open(&self) -> &OpenNodes {
        match self {
            TreeStage::Browsing { open, .. } | TreeStage::Selected { open, .. } => open,
        }
    }

    /// The find that is up, when one is.
    #[must_use]
    pub fn find(&self) -> Option<&StageFind> {
        match self {
            TreeStage::Browsing { find, .. } | TreeStage::Selected { find, .. } => find.as_ref(),
        }
    }

    /// How large the rows are drawn.
    #[must_use]
    pub fn scale(&self) -> ContentScale {
        match self {
            TreeStage::Browsing { scale, .. } | TreeStage::Selected { scale, .. } => *scale,
        }
    }

    /// The row the cursor is on, when there is one.
    #[must_use]
    pub fn row(&self) -> Option<RowNo> {
        match self {
            TreeStage::Browsing { .. } => None,
            TreeStage::Selected { row, .. } => Some(*row),
        }
    }
}

impl Default for TreeStage {
    fn default() -> Self {
        TreeStage::Browsing {
            open: OpenNodes::top_level(),
            find: None,
            scale: ContentScale::ACTUAL,
            edits: DataEdits::default(),
        }
    }
}

/// What the stage carries whichever row the cursor is on: the find that is up and the zoom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Look {
    pub(super) find: Option<StageFind>,
    pub(super) scale: ContentScale,
    pub(super) edits: DataEdits,
}

/// What moves the stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeIn {
    /// Open the node at this path if it is closed, close it if it is open.
    Toggle(TreePath),
    /// Open the node at this path.
    Open(TreePath),
    /// Close the node at this path.
    Close(TreePath),
    /// Close everything but the top level.
    CollapseAll,
    /// The cursor moved to this row.
    Select(RowNo),
    /// Move the cursor along the visible rows: the arrow keys, Page Up and Down, Home and End.
    Move(RowStep),
    /// Put the cursor away.
    Deselect,
    /// Open these nodes (the ones above a hit) and put the cursor on `row`, which is where the
    /// hit is once they are open.
    Show {
        /// The nodes to open.
        opened: Vec<TreePath>,
        /// The row to put the cursor on.
        row: RowNo,
    },
    /// Search the document for this text; empty text puts the hits away.
    Find(TypedText),
    /// The search for `query` found `count` hits, `nearest` being the one the reader meets first.
    Results {
        /// The text searched for.
        query: TypedText,
        /// How many hits there are.
        count: HitCount,
        /// The hit closest to the reader.
        nearest: HitIndex,
    },
    /// The next hit, wrapping to the first after the last.
    NextHit,
    /// The previous hit, wrapping to the last before the first.
    PreviousHit,
    /// Make this hit the current one and show it (a row of the palette's hits).
    GoToHit(HitIndex),
    /// Close the find.
    CloseFind,
    /// Draw the rows one step larger or smaller.
    ZoomStep(ZoomDir),
    /// Draw the rows at actual size.
    ZoomActual,
    /// Typing into the value under the cursor, or its key: begun by Return, taken or let go.
    Typing(TypingIn),
    /// Shift-Return: type a new key for the node under the cursor.
    RenameKey,
    /// Add a node below the one under the cursor.
    AddItem,
    /// Take away the node under the cursor.
    DeleteItem,
    /// The edits made the rows different: these nodes are open and the cursor is on this row.
    Reshaped {
        /// The nodes that are open now.
        open: OpenNodes,
        /// The row the cursor is on now.
        row: Option<RowNo>,
    },
    /// The edits as a whole: undo, redo, save, let go, and what the window says of them.
    Session(EditSessionIn),
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for TreeIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        TreeIn::Elapsed
    }
}

/// What the stage asks of the window: its search. Which nodes are open is not kept for next time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOut {
    /// Something for the search: run it, show a hit, clear the marks.
    Find(FindOut),
    /// Make this text the value of the node at this row (a place among the visible rows).
    Set {
        /// The row.
        row: RowNo,
        /// What the value is from now on, as typed.
        text: TypedText,
    },
    /// Make this text the key of the node at this row.
    Rename {
        /// The row.
        row: RowNo,
        /// The key from now on.
        text: TypedText,
    },
    /// Add a node below the one at this row.
    Add(RowNo),
    /// Take away the node at this row.
    Delete(RowNo),
    /// Something about the edits as a whole.
    Session(EditSessionOut),
}

/// What the node under the cursor holds, as far as typing into it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeUnder {
    /// There is no node under the cursor.
    #[default]
    Nothing,
    /// A string, a number, `true`, `false` or `null`: its value is typed in.
    Scalar,
    /// An object or an array, or a line that is no value: there is no text to type in.
    Container,
}

/// Whether the node under the cursor has a key to rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyUnder {
    /// It has none: the root, or an item of an array.
    #[default]
    Absent,
    /// It is an entry of an object.
    Present,
}

/// What the stage needs to know of the open file: how many rows are visible, which the open nodes
/// decide, how many fit the room, and what the node under the cursor allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TreeParams {
    /// How many rows the open nodes make visible.
    pub rows: u32,
    /// How many rows fit the room: what a page up or down moves by.
    pub page: u32,
    /// Whether the nodes can be edited and the file written back.
    pub editable: Editable,
    /// What the node under the cursor holds.
    pub under: NodeUnder,
    /// Whether the node under the cursor has a key.
    pub key: KeyUnder,
}
