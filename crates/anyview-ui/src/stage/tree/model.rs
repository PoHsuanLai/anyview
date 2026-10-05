//! The tree stage's states and inputs.

use crate::stage::row::RowNo;
use anyview_core::{OpenNodes, TreePath};

/// What the tree stage is doing. The top level of a document is open when a file opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeStage {
    /// Reading, with no row picked.
    Browsing { open: OpenNodes },
    /// Reading with the cursor on a row.
    Selected { open: OpenNodes, row: RowNo },
}

impl TreeStage {
    /// The nodes that are open.
    pub fn open(&self) -> &OpenNodes {
        match self {
            TreeStage::Browsing { open } | TreeStage::Selected { open, .. } => open,
        }
    }

    /// The row the cursor is on, when there is one.
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
        }
    }
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
    /// Put the cursor away.
    Deselect,
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for TreeIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        TreeIn::Elapsed
    }
}

/// What the stage asks of the window: nothing. Which nodes are open is not kept for next time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOut {}

/// What the stage needs to know of the open file: nothing it does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TreeParams;
