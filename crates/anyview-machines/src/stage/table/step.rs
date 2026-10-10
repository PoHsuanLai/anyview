use super::model::{SheetNo, SheetTotal, TableIn, TableOut, TableParams, TableStage};
use crate::stage::row::RowNo;
use anyview_core::ColumnSort;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (TableStage, Vec<TableOut>);

impl Machine for TableStage {
    type In = TableIn;
    type Out = TableOut;
    type Params = TableParams;
    type Ctx = ();

    fn step(self, input: TableIn, _at: Stamp, params: &TableParams, _cx: &()) -> Step {
        let (sheet, row, sort) = advance(self.sheet(), self.row(), self.sort(), input, params);
        (TableStage::of(sheet, row, sort), vec![])
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TableStage::Browsing { .. } | TableStage::Selected { .. } => None,
        }
    }
}

/// The sheet `input` asks for from `sheet`, or `None` when it asks for none or for one the file
/// does not have.
fn sheet_asked(sheet: SheetNo, input: TableIn, total: SheetTotal) -> Option<SheetNo> {
    let last = total.0.checked_sub(1)?;
    match input {
        TableIn::NextSheet => Some(SheetNo(sheet.0.saturating_add(1).min(last))),
        TableIn::PreviousSheet => Some(SheetNo(sheet.0.saturating_sub(1).min(last))),
        TableIn::ChooseSheet(chosen) if chosen.0 <= last => Some(chosen),
        TableIn::ChooseSheet(_)
        | TableIn::Select(_)
        | TableIn::Move(_)
        | TableIn::PressHeader(_)
        | TableIn::Deselect
        | TableIn::Elapsed => None,
    }
}

/// The sheet, the cursor and the sort after `input`.
fn advance(
    sheet: SheetNo,
    row: Option<RowNo>,
    sort: Option<ColumnSort>,
    input: TableIn,
    params: &TableParams,
) -> (SheetNo, Option<RowNo>, Option<ColumnSort>) {
    match input {
        // Another sheet has other rows and other columns: the cursor and the sort do not carry
        // over.
        TableIn::NextSheet | TableIn::PreviousSheet | TableIn::ChooseSheet(_) => {
            match sheet_asked(sheet, input, params.sheets).filter(|next| *next != sheet) {
                Some(next) => (next, None, None),
                None => (sheet, row, sort),
            }
        }
        TableIn::Select(picked) => (sheet, Some(picked), sort),
        TableIn::Move(step) => (
            sheet,
            step.from(row, params.rows, params.page).or(row),
            sort,
        ),
        // The rows change places, so the row the cursor was on is somewhere else.
        TableIn::PressHeader(column) if column < params.columns => {
            (sheet, None, ColumnSort::after_press(sort, column))
        }
        TableIn::PressHeader(_) | TableIn::Elapsed => (sheet, row, sort),
        TableIn::Deselect => (sheet, None, sort),
    }
}
