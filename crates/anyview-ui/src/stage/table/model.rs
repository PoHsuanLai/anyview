//! The table stage's states, inputs and parameters.

use crate::stage::row::{RowNo, RowStep};

/// A sheet of the open file, zero-based. A delimited file has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetNo(pub u32);

/// How many sheets the open file has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetTotal(pub u32);

/// What the table stage is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStage {
    /// Reading a sheet with no row picked.
    Browsing { sheet: SheetNo },
    /// Reading a sheet with the cursor on a row.
    Selected { sheet: SheetNo, row: RowNo },
}

impl TableStage {
    /// The sheet being read.
    pub fn sheet(&self) -> SheetNo {
        match self {
            TableStage::Browsing { sheet } | TableStage::Selected { sheet, .. } => *sheet,
        }
    }

    /// The row the cursor is on, when there is one.
    pub fn row(&self) -> Option<RowNo> {
        match self {
            TableStage::Browsing { .. } => None,
            TableStage::Selected { row, .. } => Some(*row),
        }
    }
}

impl Default for TableStage {
    fn default() -> Self {
        TableStage::Browsing { sheet: SheetNo(0) }
    }
}

/// What moves the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableIn {
    /// The next sheet; the last stays.
    NextSheet,
    /// The previous sheet; the first stays.
    PreviousSheet,
    /// This sheet, when the file has it.
    ChooseSheet(SheetNo),
    /// The cursor moved to this row.
    Select(RowNo),
    /// Move the cursor along the rows: the arrow keys, Page Up and Down, Home and End.
    Move(RowStep),
    /// Put the cursor away.
    Deselect,
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for TableIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        TableIn::Elapsed
    }
}

/// What the stage asks of the window: nothing. Where the person is in a table is not kept for
/// next time, so no state change has an effect to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableOut {}

/// What the stage needs to know of the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableParams {
    /// How many sheets the file has.
    pub sheets: SheetTotal,
    /// How many rows the sheet being read has.
    pub rows: u32,
    /// How many rows fit the room: what a page up or down moves by.
    pub page: u32,
}
