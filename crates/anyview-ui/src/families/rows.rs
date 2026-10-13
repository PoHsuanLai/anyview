//! The height of a row of a table or a tree. Both draw quire's compact `Row` in a `VirtualList` that
//! must be told each row's height up front, so the number is quire's own token
//! (`--row-compact-h`, `ROW_SCALE.compact_height`), never a copy of it here.

use ds::style::tokens::row_scale::ROW_SCALE;

/// A compact row's height in logical pixels.
pub(super) fn compact_px() -> f32 {
    f32::from(ROW_SCALE.compact_height.0)
}

/// A compact row's height at `scale`, in logical pixels: a zoomed table or tree draws taller or
/// shorter rows, and every count of rows to a page and every scroll offset follows.
pub(super) fn scaled_px(scale: crate::ContentScale) -> f32 {
    compact_px() * scale.factor()
}
