//! The height of a row of a table or a tree. Both draw quire's compact `Row` in a `VirtualList` that
//! must be told each row's height up front, so the number is quire's own token
//! (`--row-compact-h`, `ROW_SCALE.compact_height`), never a copy of it here.

use ds::style::tokens::row_scale::ROW_SCALE;

/// A compact row's height in logical pixels.
pub(super) fn compact_px() -> f32 {
    f32::from(ROW_SCALE.compact_height.0)
}
