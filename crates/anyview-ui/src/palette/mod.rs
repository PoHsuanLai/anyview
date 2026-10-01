//! The ⌘K palette: a query, a selection over rows someone else ranked, and what Enter runs.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Palette, PaletteIn, PaletteMove, PaletteOut, PaletteParams, RowIndex};
