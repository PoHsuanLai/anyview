//! Where a find's hits are in a table and in a tree: the places the stage's view marks and the
//! palette lists. A text's hits are lines and byte ranges (`anyview_text::FindHit`); a table's are
//! cells and a tree's are nodes.

use anyview_core::TreePath;

/// One hit in a table: the cell, and the match in its text as byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellHit {
    /// The row of the file, counting the rows under the header from zero.
    pub row: u32,
    /// The column, from zero.
    pub column: u32,
    /// Where the match starts in the cell's text.
    pub from: u32,
    /// Where the match ends.
    pub to: u32,
}

/// Which part of a row of a tree a hit is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodePart {
    /// The key the node is under.
    Key,
    /// The value the row shows for the node.
    Value,
}

/// One hit in a tree: the node, the part of its row, and the match in that part's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeHit {
    /// Where the node is.
    pub path: TreePath,
    /// The part of its row the match is in.
    pub part: NodePart,
    /// Where the match starts in the part's text.
    pub from: u32,
    /// Where the match ends.
    pub to: u32,
    /// The keys above the node, as the palette says where it is: `users › 3`.
    pub trail: String,
}
