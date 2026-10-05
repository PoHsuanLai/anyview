//! A row of a list the stage shows, counted from the top.

/// A row's position among the rows a table or a tree shows, zero-based. A tree's rows are its
/// visible nodes, so the same position names another node once something above it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RowNo(pub u32);
