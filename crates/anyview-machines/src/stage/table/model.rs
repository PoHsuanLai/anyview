//! The table stage's states, inputs and parameters.

use super::column::ColumnSort;
use crate::stage::find::{FindOut, HitCount, HitIndex, StageFind};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::zoom::{ContentScale, ZoomDir};
use crate::typed::TypedText;

/// A sheet of the open file, zero-based. A delimited file has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetNo(pub u32);

/// How many sheets the open file has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetTotal(pub u32);

/// What the table stage is doing. The sort is only a way of looking at the rows: the rows the
/// cursor and the sort name are positions in the order the view shows, and the file is never
/// reordered. A find that is up searches the sheet being read, and the hit it is on is the row
/// under the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableStage {
    /// Reading a sheet with no row picked.
    Browsing {
        /// The sheet being read.
        sheet: SheetNo,
        /// How the rows are ordered, when they are.
        sort: Option<ColumnSort>,
        /// The find that is up, when one is.
        find: Option<StageFind>,
        /// How large the rows are drawn.
        scale: ContentScale,
    },
    /// Reading a sheet with the cursor on a row.
    Selected {
        /// The sheet being read.
        sheet: SheetNo,
        /// The row under the cursor.
        row: RowNo,
        /// How the rows are ordered, when they are.
        sort: Option<ColumnSort>,
        /// The find that is up, when one is.
        find: Option<StageFind>,
        /// How large the rows are drawn.
        scale: ContentScale,
    },
}

impl TableStage {
    /// The stage with the cursor on `row`, or on none.
    pub(super) fn of(
        sheet: SheetNo,
        row: Option<RowNo>,
        sort: Option<ColumnSort>,
        look: Look,
    ) -> Self {
        let Look { find, scale } = look;
        match row {
            Some(row) => TableStage::Selected {
                sheet,
                row,
                sort,
                find,
                scale,
            },
            None => TableStage::Browsing {
                sheet,
                sort,
                find,
                scale,
            },
        }
    }

    /// The find and the zoom this stage carries.
    pub(super) fn look(&self) -> Look {
        match self {
            TableStage::Browsing { find, scale, .. } | TableStage::Selected { find, scale, .. } => {
                Look {
                    find: find.clone(),
                    scale: *scale,
                }
            }
        }
    }

    /// The find that is up, when one is.
    #[must_use]
    pub fn find(&self) -> Option<&StageFind> {
        match self {
            TableStage::Browsing { find, .. } | TableStage::Selected { find, .. } => find.as_ref(),
        }
    }

    /// How large the rows are drawn.
    #[must_use]
    pub fn scale(&self) -> ContentScale {
        match self {
            TableStage::Browsing { scale, .. } | TableStage::Selected { scale, .. } => *scale,
        }
    }

    /// The sheet being read.
    #[must_use]
    pub fn sheet(&self) -> SheetNo {
        match self {
            TableStage::Browsing { sheet, .. } | TableStage::Selected { sheet, .. } => *sheet,
        }
    }

    /// The row the cursor is on, when there is one.
    #[must_use]
    pub fn row(&self) -> Option<RowNo> {
        match self {
            TableStage::Browsing { .. } => None,
            TableStage::Selected { row, .. } => Some(*row),
        }
    }

    /// The column the rows are sorted by, when they are.
    #[must_use]
    pub fn sort(&self) -> Option<ColumnSort> {
        match self {
            TableStage::Browsing { sort, .. } | TableStage::Selected { sort, .. } => *sort,
        }
    }
}

impl Default for TableStage {
    fn default() -> Self {
        TableStage::Browsing {
            sheet: SheetNo(0),
            sort: None,
            find: None,
            scale: ContentScale::ACTUAL,
        }
    }
}

/// What the stage carries whichever row the cursor is on: the find that is up and the zoom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Look {
    pub(super) find: Option<StageFind>,
    pub(super) scale: ContentScale,
}

/// What moves the stage.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// A press on this column's header (zero-based, among the file's columns): sort by it, flip
    /// the sort, or put the sort away. The cursor goes with the old order.
    PressHeader(u32),
    /// Put the cursor away.
    Deselect,
    /// Search the sheet for this text; empty text puts the hits away.
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
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for TableIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        TableIn::Elapsed
    }
}

/// What the stage asks of the window: its search. Where the person is in a table is not kept for
/// next time, so no move of the cursor has an effect to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableOut {
    /// Something for the search: run it, show a hit, clear the marks.
    Find(FindOut),
}

/// What the stage needs to know of the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableParams {
    /// How many sheets the file has.
    pub sheets: SheetTotal,
    /// How many columns the sheet being read has.
    pub columns: u32,
    /// How many rows the sheet being read has.
    pub rows: u32,
    /// How many rows fit the room: what a page up or down moves by.
    pub page: u32,
}
