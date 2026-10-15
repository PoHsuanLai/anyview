//! The table stage's cells: the cell cursor within the picked row, typing into a cell, and the
//! edits that add or take away rows. What a change does to the file is the window's to carry out:
//! the stage names the row and column (places in the order shown) and hears back what the edits
//! came to.

use super::column::ColumnSort;
use super::model::{CellFocus, CellStep, Cells, ColumnNo, TableIn, TableOut, TableParams};
use crate::stage::data_edits::{self, AfterTyping, Taken, TypingIn, TypingSeed};
use crate::stage::row::RowNo;
use crate::stage::text::Editable;
use ds_core::selection::RowSelection;

/// What an input about cells left: the picked rows, the sort, the cells, and what is asked.
pub(super) struct Worked {
    pub(super) rows: RowSelection,
    pub(super) sort: Option<ColumnSort>,
    pub(super) cells: Cells,
    pub(super) outs: Vec<TableOut>,
}

/// The row the cursor is on, among `rows`.
fn cursor_row(rows: &RowSelection) -> Option<RowNo> {
    let at = rows.cursor().or_else(|| rows.rows().next())?;
    u32::try_from(at).ok().map(RowNo)
}

/// `column` held inside the sheet's columns.
fn within(column: ColumnNo, params: &TableParams) -> ColumnNo {
    ColumnNo(column.0.min(params.columns.saturating_sub(1)))
}

/// The column the cell cursor is on: the first when it is on the row.
fn column_of(cells: &Cells) -> ColumnNo {
    match cells.focus {
        CellFocus::Row => ColumnNo(0),
        CellFocus::Cell(column) => column,
    }
}

/// The cells, rows and sort after `input`, which is one of the inputs about cells.
pub(super) fn apply(
    (rows, sort, cells): (RowSelection, Option<ColumnSort>, Cells),
    input: TableIn,
    params: &TableParams,
) -> Worked {
    let still = |rows: RowSelection, cells: Cells| Worked {
        rows,
        sort,
        cells,
        outs: Vec::new(),
    };
    let row = cursor_row(&rows);
    match (input, row, params.editable) {
        (TableIn::Typing(TypingIn::Begin(seed)), Some(_), Editable::Yes) => {
            begin(rows, sort, cells, seed, params)
        }
        (TableIn::Typing(TypingIn::Begin(_)), _, _) => still(rows, cells),
        (TableIn::Typing(input), row, editable) => {
            taken(rows, sort, cells, input, (row, editable), params)
        }
        (TableIn::EditAt(at, column), _, Editable::Yes) if at.0 < params.rows => {
            let rows = RowSelection::single(at.0 as usize); // a u32 fits a usize
            begin(rows, sort, cells, TypingSeed::Existing, params).focused(within(column, params))
        }
        (TableIn::FocusCell(at, column), _, Editable::Yes) if at.0 < params.rows => {
            let rows = RowSelection::single(at.0 as usize); // a u32 fits a usize
            let cells = Cells {
                focus: CellFocus::Cell(within(column, params)),
                ..cells
            };
            still(rows, cells)
        }
        (TableIn::StepCell(step), Some(_), Editable::Yes) => {
            let column = column_of(&cells);
            let next = match (cells.focus, step) {
                (CellFocus::Row, _) => ColumnNo(0),
                (CellFocus::Cell(_), CellStep::Left) => ColumnNo(column.0.saturating_sub(1)),
                (CellFocus::Cell(_), CellStep::Right) => within(ColumnNo(column.0 + 1), params),
            };
            let cells = Cells {
                focus: CellFocus::Cell(next),
                ..cells
            };
            still(rows, cells)
        }
        // Rows come and go in the file's own order: the sort is put away, and a row is picked
        // once the window says where the edit left the rows.
        (TableIn::AddRow, Some(at), Editable::Yes) => Worked {
            rows,
            sort: None,
            cells,
            outs: vec![TableOut::AddRow { below: at, sort }],
        },
        (TableIn::DeleteRows, Some(_), Editable::Yes) => {
            let outs = vec![TableOut::DeleteRows { rows, sort }];
            Worked {
                rows: RowSelection::none(),
                sort: None,
                cells,
                outs,
            }
        }
        (TableIn::Session(input), _, _) => {
            let Cells { focus, edits } = cells;
            let (edits, out) = data_edits::session(edits, input);
            Worked {
                rows,
                sort,
                cells: Cells { focus, edits },
                outs: out.map(TableOut::Session).into_iter().collect(),
            }
        }
        (
            TableIn::EditAt(..)
            | TableIn::FocusCell(..)
            | TableIn::StepCell(_)
            | TableIn::AddRow
            | TableIn::DeleteRows,
            _,
            _,
        ) => still(rows, cells),
        // The inputs about the rows and the find are not about cells.
        (
            TableIn::NextSheet
            | TableIn::PreviousSheet
            | TableIn::ChooseSheet(_)
            | TableIn::Select(_)
            | TableIn::Pick(_)
            | TableIn::SelectAll
            | TableIn::Move(_)
            | TableIn::PressHeader(_)
            | TableIn::Reseat(_)
            | TableIn::Deselect
            | TableIn::Copy
            | TableIn::Find(_)
            | TableIn::Results { .. }
            | TableIn::NextHit
            | TableIn::PreviousHit
            | TableIn::GoToHit(_)
            | TableIn::CloseFind
            | TableIn::ZoomStep(_)
            | TableIn::ZoomActual
            | TableIn::Elapsed,
            _,
            _,
        ) => still(rows, cells),
    }
}

impl Worked {
    /// The same with the cell cursor on `column`.
    fn focused(self, column: ColumnNo) -> Worked {
        Worked {
            cells: Cells {
                focus: CellFocus::Cell(column),
                ..self.cells
            },
            ..self
        }
    }
}

/// Typing begins in the cell under the cursor: the first column when the cursor is on the row.
fn begin(
    rows: RowSelection,
    sort: Option<ColumnSort>,
    cells: Cells,
    seed: TypingSeed,
    params: &TableParams,
) -> Worked {
    let column = within(column_of(&cells), params);
    let (edits, _) = data_edits::typing(cells.edits, TypingIn::Begin(seed), params.editable);
    Worked {
        rows,
        sort,
        cells: Cells {
            focus: CellFocus::Cell(column),
            edits,
        },
        outs: Vec::new(),
    }
}

/// What typing came to: a commit sets the cell and moves the cursor on; a cancel lets it go.
fn taken(
    rows: RowSelection,
    sort: Option<ColumnSort>,
    cells: Cells,
    input: TypingIn,
    (row, editable): (Option<RowNo>, Editable),
    params: &TableParams,
) -> Worked {
    let column = column_of(&cells);
    let (edits, taken) = data_edits::typing(cells.edits.clone(), input, editable);
    let mut cells = Cells { edits, ..cells };
    let mut rows = rows;
    let mut outs = Vec::new();
    match (taken, row) {
        (Taken::Value(text, then), Some(at)) | (Taken::Key(text, then), Some(at)) => {
            outs.push(TableOut::Set {
                row: at,
                column,
                text,
                sort,
            });
            match then {
                AfterTyping::Stay => {}
                // The last row and the last cell stay where they are.
                AfterTyping::Down => {
                    if at.0.saturating_add(1) < params.rows {
                        rows = RowSelection::single(at.0 as usize + 1); // a u32 fits a usize
                    }
                }
                AfterTyping::Right => {
                    cells.focus = CellFocus::Cell(within(ColumnNo(column.0 + 1), params));
                }
            }
        }
        (Taken::Value(..) | Taken::Key(..) | Taken::Nothing, _) => {}
    }
    Worked {
        rows,
        sort,
        cells,
        outs,
    }
}
