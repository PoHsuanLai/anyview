//! The table stage's transitions.

use super::model::{SheetNo, SheetTotal, TableIn, TableOut, TableParams, TableStage};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (TableStage, Vec<TableOut>);

impl Machine for TableStage {
    type In = TableIn;
    type Out = TableOut;
    type Params = TableParams;
    type Ctx = ();

    fn step(self, input: TableIn, _at: Stamp, params: &TableParams, _cx: &()) -> Step {
        match self {
            TableStage::Browsing { sheet } => browsing(sheet, input, params),
            TableStage::Selected { sheet, row } => selected(sheet, row, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TableStage::Browsing { sheet: _ } | TableStage::Selected { sheet: _, row: _ } => None,
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
        TableIn::ChooseSheet(_) | TableIn::Select(_) | TableIn::Deselect | TableIn::Elapsed => None,
    }
}

fn browsing(sheet: SheetNo, input: TableIn, params: &TableParams) -> Step {
    match input {
        TableIn::NextSheet | TableIn::PreviousSheet | TableIn::ChooseSheet(_) => {
            let sheet = sheet_asked(sheet, input, params.sheets).unwrap_or(sheet);
            (TableStage::Browsing { sheet }, vec![])
        }
        TableIn::Select(row) => (TableStage::Selected { sheet, row }, vec![]),
        TableIn::Deselect | TableIn::Elapsed => (TableStage::Browsing { sheet }, vec![]),
    }
}

fn selected(
    sheet: SheetNo,
    row: crate::stage::row::RowNo,
    input: TableIn,
    params: &TableParams,
) -> Step {
    match input {
        // Another sheet has other rows: the cursor does not carry over.
        TableIn::NextSheet | TableIn::PreviousSheet | TableIn::ChooseSheet(_) => {
            match sheet_asked(sheet, input, params.sheets).filter(|next| *next != sheet) {
                Some(next) => (TableStage::Browsing { sheet: next }, vec![]),
                None => (TableStage::Selected { sheet, row }, vec![]),
            }
        }
        TableIn::Select(row) => (TableStage::Selected { sheet, row }, vec![]),
        TableIn::Deselect => (TableStage::Browsing { sheet }, vec![]),
        TableIn::Elapsed => (TableStage::Selected { sheet, row }, vec![]),
    }
}
