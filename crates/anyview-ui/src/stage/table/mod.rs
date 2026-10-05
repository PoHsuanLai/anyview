//! The table stage: the sheet being read and the row the cursor is on.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{SheetNo, SheetTotal, TableIn, TableOut, TableParams, TableStage};
