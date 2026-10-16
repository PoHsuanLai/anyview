//! The table stage: the sheet being read and the row the cursor is on.

mod cells;
mod column;
mod command;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use column::{ColumnSort, SortDirection, is_numeric, sorted_rows};
pub(super) use command::input_for;
pub use model::{
    CellFocus, CellStep, Cells, ColumnNo, SheetNo, SheetTotal, TableIn, TableOut, TableParams,
    TableStage,
};
