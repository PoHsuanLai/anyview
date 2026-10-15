//! The table's edits as the window carries them out: what the stage asks of the cells (a cell
//! typed in, a row added, rows taken away, an edit taken back) is done to the document's edits, and
//! the stage is told what they stand at and where its cursor goes. The rows the stage names are
//! places in the order shown; a sort is a way of looking, so a place is turned into the row of the
//! file it shows before anything is changed. A save hands a worker the edits to build the file
//! from.

use super::doc::TableDoc;
use super::sort::SortedOrder;
use crate::families::view::{EditDone, SaveBytes};
use crate::io::Job;
use crate::{
    Changes, ColumnSort, EditDepth, EditLog, EditRevision, EditSessionIn, EditSessionOut, Rewind,
    RowNo, RowOrder, SheetNo, Stage, StageIn, StageOut, TableIn, TableOut, Ticket,
};
use anyview_text::{RowRef, Table, TableEdits, Undid};
use std::sync::Arc;

/// What the stage is told of the edits `edits` holds.
pub(super) fn log_of(edits: &TableEdits) -> EditLog {
    let count = |held: usize| EditDepth(u32::try_from(held).unwrap_or(u32::MAX));
    EditLog {
        changes: if edits.is_modified() {
            Changes::Unsaved
        } else {
            Changes::Saved
        },
        undo: count(edits.undoable()),
        redo: count(edits.redoable()),
        rows: if edits.is_reshaped() {
            RowOrder::Changed
        } else {
            RowOrder::AsFile
        },
        revision: EditRevision(edits.stamp().number()),
    }
}

fn logged(edits: &TableEdits) -> StageIn {
    StageIn::Table(TableIn::Session(EditSessionIn::Logged(log_of(edits))))
}

/// The row shown at `place` under `order` (the file's own order when `None`).
pub(super) fn row_at(edits: &TableEdits, order: Option<&[u32]>, place: usize) -> Option<RowRef> {
    match order {
        Some(order) => order.get(place).map(|file| RowRef::File(*file)),
        None => edits.row_at(u32::try_from(place).ok()?),
    }
}

/// The text of the cell at `column` of the row shown at `place`.
pub(super) fn text_at(
    (edits, table): (&TableEdits, &Table),
    order: Option<&[u32]>,
    place: usize,
    column: usize,
) -> String {
    match (row_at(edits, order, place), u32::try_from(column)) {
        (Some(row), Ok(column)) => edits.cell(table, row, column),
        (None, _) | (_, Err(_)) => String::new(),
    }
}

/// Where the row shown at `place` is among the rows of the edits.
fn position_of(edits: &TableEdits, order: Option<&[u32]>, place: usize) -> Option<u32> {
    match order {
        Some(order) => edits.position_of(RowRef::File(*order.get(place)?)),
        None => u32::try_from(place).ok(),
    }
}

/// The stage told where its cursor goes once the rows are `edits`: on `row`, held inside them.
fn cursor_on(edits: &TableEdits, row: Option<u32>) -> Option<StageIn> {
    let row = row?;
    Some(StageIn::Table(match edits.rows() {
        0 => TableIn::Deselect,
        total => TableIn::Select(RowNo(row.min(total - 1))),
    }))
}

fn told(tell: Vec<StageIn>) -> EditDone {
    EditDone {
        tell,
        write: SaveBytes::Nothing,
    }
}

/// The order the rows were named in: the sort of the output, when the worker's order is of it.
fn named_in<'a>(order: Option<&'a SortedOrder>, sort: &Option<ColumnSort>) -> Option<&'a [u32]> {
    order
        .filter(|order| order.sheet == SheetNo(0) && Some(order.sort) == *sort)
        .map(|order| order.order.as_slice())
}

/// Carry out what the stage asked of the edits.
pub(super) fn carry(
    doc: &Arc<TableDoc>,
    ticket: Ticket,
    stage: &Stage,
    out: &StageOut,
    order: Option<&SortedOrder>,
) -> EditDone {
    let (Stage::Table(shown), StageOut::Table(out)) = (stage, out) else {
        return EditDone::default();
    };
    let Some(sheet) = doc.sheets.first() else {
        return EditDone::default();
    };
    let table = &sheet.table;
    let mut held = doc.edits();
    let Some(edits) = held.as_mut() else {
        return EditDone::default();
    };
    let cursor = shown.row().map(|row| row.0);
    match out {
        TableOut::Set {
            row,
            column,
            text,
            sort,
        } => {
            if let Some(target) = row_at(edits, named_in(order, sort), row.0 as usize) {
                // a u32 fits a usize
                edits.set_cell(table, target, column.0, text.as_str());
            }
            told(vec![logged(edits)])
        }
        TableOut::AddRow { below, sort } => {
            let Some(position) = position_of(edits, named_in(order, sort), below.0 as usize) else {
                // a u32 fits a usize
                return EditDone::default();
            };
            let at = edits.insert_below(position);
            told(vec![
                logged(edits),
                StageIn::Table(TableIn::Select(RowNo(at))),
            ])
        }
        TableOut::DeleteRows { rows, sort } => {
            let order = named_in(order, sort);
            let places: Vec<u32> = rows
                .rows()
                .filter_map(|place| position_of(edits, order, place))
                .collect();
            edits.delete(&places);
            let first = places.iter().min().copied().unwrap_or(0);
            let mut tell = vec![logged(edits)];
            tell.extend(cursor_on(edits, Some(first)));
            told(tell)
        }
        TableOut::Session(EditSessionOut::Rewind(way)) => {
            let done = match way {
                Rewind::Undo => edits.undo(),
                Rewind::Redo => edits.redo(),
            };
            match done {
                Undid::Something => {
                    let mut tell = vec![logged(edits)];
                    tell.extend(cursor_on(edits, cursor));
                    told(tell)
                }
                Undid::Nothing => EditDone::default(),
            }
        }
        TableOut::Session(EditSessionOut::Save) => EditDone {
            tell: Vec::new(),
            write: SaveBytes::Building(Job::DataBytes {
                ticket,
                doc: Arc::clone(doc),
                edits: Box::new(edits.clone()),
                stamp: edits.stamp(),
            }),
        },
        TableOut::Session(EditSessionOut::Discard) => {
            // Back to the state the file holds: taken back as far as the last save, or done again
            // up to it if the person had gone back past it.
            while edits.is_modified() && edits.undo() == Undid::Something {}
            while edits.is_modified() && edits.redo() == Undid::Something {}
            let mut tell = vec![logged(edits)];
            tell.extend(cursor_on(edits, cursor));
            told(tell)
        }
        TableOut::Find(_) | TableOut::Copy(_) => EditDone::default(),
    }
}
