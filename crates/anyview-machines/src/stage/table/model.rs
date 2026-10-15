//! The table stage's states, inputs and parameters.

use super::column::ColumnSort;
use crate::stage::data_edits::{DataEdits, EditSessionIn, EditSessionOut, TypingIn, TypingState};
use crate::stage::find::{FindOut, HitCount, HitIndex, StageFind};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::text::Editable;
use crate::stage::zoom::{ContentScale, ZoomDir};
use crate::typed::TypedText;
use ds_core::selection::{Pick, RowSelection};

/// A sheet of the open file, zero-based. A delimited file has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetNo(pub u32);

/// How many sheets the open file has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SheetTotal(pub u32);

/// A column of the sheet, zero-based, among the file's columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ColumnNo(pub u32);

/// Whether the cursor is on a whole row or on one cell of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellFocus {
    /// On the row: the arrow keys move between rows.
    #[default]
    Row,
    /// On this cell of the row: Left and Right move between the cells.
    Cell(ColumnNo),
}

/// The cell cursor and the edits of a delimited sheet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cells {
    /// Where the cursor is within its row.
    pub focus: CellFocus,
    /// What is typed in, and what the window says of the edits.
    pub edits: DataEdits,
}

impl Cells {
    /// The cursor on the row, with nothing edited.
    pub const NONE: Cells = Cells {
        focus: CellFocus::Row,
        edits: DataEdits::NONE,
    };
}

/// Which way the cell cursor steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStep {
    /// To the cell before.
    Left,
    /// To the cell after.
    Right,
}

/// What the table stage is doing. The sort is only a way of looking at the rows: the rows the
/// selection and the sort name are positions in the order the view shows, and the file is never
/// reordered. A find that is up searches the sheet being read, and the hit it is on is the row
/// under the cursor. The selection is quire's `RowSelection`: a few ranges however many rows are
/// picked, with an anchor for Shift and the cursor the keys rest on.
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
        /// The cell cursor and the edits; no cell is under a cursor with no row.
        cells: Cells,
    },
    /// Reading a sheet with one row or several picked; never an empty selection.
    Selected {
        /// The sheet being read.
        sheet: SheetNo,
        /// The rows picked, and the cursor among them.
        rows: RowSelection,
        /// How the rows are ordered, when they are.
        sort: Option<ColumnSort>,
        /// The find that is up, when one is.
        find: Option<StageFind>,
        /// How large the rows are drawn.
        scale: ContentScale,
        /// The cell cursor and the edits.
        cells: Cells,
    },
}

impl TableStage {
    /// The stage with `rows` picked, or with none picked when `rows` is empty.
    pub(super) fn of(
        sheet: SheetNo,
        rows: RowSelection,
        sort: Option<ColumnSort>,
        look: Look,
    ) -> Self {
        let Look { find, scale, cells } = look;
        match rows.is_empty() {
            false => TableStage::Selected {
                sheet,
                rows,
                sort,
                find,
                scale,
                cells,
            },
            // With no row under the cursor there is no cell, and nothing is typed in.
            true => TableStage::Browsing {
                sheet,
                sort,
                find,
                scale,
                cells: Cells {
                    focus: CellFocus::Row,
                    edits: DataEdits {
                        typing: TypingState::Idle,
                        ..cells.edits
                    },
                },
            },
        }
    }

    /// The find and the zoom this stage carries.
    pub(super) fn look(&self) -> Look {
        match self {
            TableStage::Browsing {
                find, scale, cells, ..
            }
            | TableStage::Selected {
                find, scale, cells, ..
            } => Look {
                find: find.clone(),
                scale: *scale,
                cells: cells.clone(),
            },
        }
    }

    /// The cell cursor and the edits.
    #[must_use]
    pub fn cells(&self) -> &Cells {
        match self {
            TableStage::Browsing { cells, .. } | TableStage::Selected { cells, .. } => cells,
        }
    }

    /// The column the cell cursor is on, when it is on a cell and not a whole row.
    #[must_use]
    pub fn column(&self) -> Option<ColumnNo> {
        match self.cells().focus {
            CellFocus::Row => None,
            CellFocus::Cell(column) => Some(column),
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

    /// The row the cursor is on, when there is one: the one picked last, or the first picked
    /// when the selection came to be some other way.
    #[must_use]
    pub fn row(&self) -> Option<RowNo> {
        let rows = self.selection()?;
        let at = rows.cursor().or_else(|| rows.rows().next())?;
        u32::try_from(at).ok().map(RowNo)
    }

    /// The rows picked, when any are.
    #[must_use]
    pub fn selection(&self) -> Option<&RowSelection> {
        match self {
            TableStage::Browsing { .. } => None,
            TableStage::Selected { rows, .. } => Some(rows),
        }
    }

    /// How many rows are picked.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selection().map_or(0, RowSelection::count)
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
            cells: Cells::default(),
        }
    }
}

/// What the stage carries whichever row the cursor is on: the find that is up, the zoom, and the
/// cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Look {
    pub(super) find: Option<StageFind>,
    pub(super) scale: ContentScale,
    pub(super) cells: Cells,
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
    /// The cursor moved to this row, which alone is picked: a find landing on a hit.
    Select(RowNo),
    /// A row was reached by a click or a key, and how: it replaces the picked rows, is toggled
    /// in them, or extends them from the anchor. A row the sheet does not have is ignored.
    Pick(Pick),
    /// Pick every row of the sheet.
    SelectAll,
    /// Move the cursor along the rows: the arrow keys, Page Up and Down, Home and End. The row it
    /// reaches alone is picked.
    Move(RowStep),
    /// A press on this column's header (zero-based, among the file's columns): sort by it, flip
    /// the sort, or put the sort away. The picked rows stay where they are until the view, whose
    /// order it is, says where the sort put them (`Reseat`).
    PressHeader(u32),
    /// The sort changed the order the rows are shown in: the rows picked are these places now.
    /// Heard only while rows are picked.
    Reseat(RowSelection),
    /// Esc: several picked rows narrow to the cursor's row, and one row is put away.
    Deselect,
    /// Copy the picked rows; heard only while rows are picked.
    Copy,
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
    /// Typing into the cell under the cursor: begun by Return or a key, taken or let go.
    Typing(TypingIn),
    /// A double-click on this cell: its row alone is picked and its text is typed into.
    EditAt(RowNo, ColumnNo),
    /// A click on this cell: its row alone is picked and the cell cursor goes to it.
    FocusCell(RowNo, ColumnNo),
    /// Left or Right: the cell cursor steps along the row.
    StepCell(CellStep),
    /// Add an empty row below the cursor's row.
    AddRow,
    /// Take away the picked rows.
    DeleteRows,
    /// The edits as a whole: undo, redo, save, let go, and what the window says of them.
    Session(EditSessionIn),
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for TableIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        TableIn::Elapsed
    }
}

/// What the stage asks of the window: its search, and the clipboard. Where the person is in a
/// table is not kept for next time, so no move of the cursor has an effect to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableOut {
    /// Something for the search: run it, show a hit, clear the marks.
    Find(FindOut),
    /// Put these picked rows (places in the order shown) on the clipboard as text.
    Copy(RowSelection),
    /// Make this text the cell at this row (a place in the order shown) and column.
    Set {
        /// The row.
        row: RowNo,
        /// The column.
        column: ColumnNo,
        /// What the cell holds from now on.
        text: TypedText,
        /// The sort the rows were in when the row was named: the row is a place in that order.
        sort: Option<ColumnSort>,
    },
    /// Add an empty row below this one (a place in the order shown).
    AddRow {
        /// The row.
        below: RowNo,
        /// The sort the rows were in when the row was named.
        sort: Option<ColumnSort>,
    },
    /// Take away these rows (places in the order shown).
    DeleteRows {
        /// The rows.
        rows: RowSelection,
        /// The sort the rows were in when they were named.
        sort: Option<ColumnSort>,
    },
    /// Something about the edits as a whole.
    Session(EditSessionOut),
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
    /// Whether the cells can be edited and the file written back.
    pub editable: Editable,
}
