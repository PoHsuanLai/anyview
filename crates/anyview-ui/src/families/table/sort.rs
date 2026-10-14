//! A sheet's sort, made on a worker: the order the rows take when the view is sorted by a column.
//! Reading the column of a big file parses it, so the window never waits for it; the view keeps
//! the rows in the order they are in until this arrives.

use super::doc::TableDoc;
use crate::{ColumnSort, SheetNo, sorted_rows};
use anyview_core::work::{Stop, StopState};

/// The order a sort gave a sheet: each entry is a row of the file.
#[derive(Debug, PartialEq, Eq)]
pub struct SortedOrder {
    /// The sheet that was sorted.
    pub sheet: SheetNo,
    /// The sort that made the order.
    pub sort: ColumnSort,
    /// The rows of the sheet, in the sorted order.
    pub order: Vec<u32>,
}

/// The order `sort` gives `sheet` of `doc`: blocking, reads the column once. `None` when there is
/// no such sheet or the load was left meanwhile.
pub(crate) fn sort_sheet(
    doc: &TableDoc,
    sheet: SheetNo,
    sort: ColumnSort,
    stop: &Stop,
) -> Option<SortedOrder> {
    let one = doc.sheets.get(sheet.0 as usize)?; // a u32 fits a usize
    let column = sort.column as usize; // a u32 fits a usize
    let cells = one
        .table
        .column(column)
        .take_while(|_| stop.stopped() != StopState::Stopped);
    let order = sorted_rows(sort.direction, cells);
    (stop.stopped() != StopState::Stopped).then_some(SortedOrder { sheet, sort, order })
}
