//! The edits made to a table, held on top of it. The table is never copied: a changed cell is one
//! entry, a row added is one segment, and a run of rows taken away is one gap in the layout of the
//! file's rows, so a million-row table costs what was changed. The history is the steps, each with
//! what undoes it.

use super::{RowIndex, Table};
use crate::edit::{EditStamp, Log};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

/// A row of the table being edited: a row of the file, or one the person added. A row keeps its
/// name wherever it moves to, so an edit to it survives rows being added or taken away above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RowRef {
    /// The row of the file at this position, counted from the first data row.
    File(u32),
    /// The row added with this number.
    New(u32),
}

/// A run of the rows as they are now.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    /// File rows `from..from + len`.
    File { from: u32, len: u32 },
    /// One added row.
    New(u32),
}

/// One step of the history. Applying it backwards undoes it.
#[derive(Debug, Clone)]
enum Step {
    /// The text of a cell: `before` and `after` are the overrides it had (none: the file's own).
    Cell {
        row: RowRef,
        column: u32,
        before: Option<String>,
        after: Option<String>,
    },
    /// Rows added or taken away: the layout before and after.
    Layout { before: Vec<Seg>, after: Vec<Seg> },
}

/// What writing the edited table does with each run of rows, in the order the file has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Op {
    /// These file rows are as they were.
    Keep(Range<u32>),
    /// This file row has these cells changed (column and text).
    Edit(u32, BTreeMap<u32, String>),
    /// A row that was added, with its cells.
    Insert(Vec<String>),
}

/// The edits to one table: cells changed, rows added and rows taken away, with undo and redo.
#[derive(Debug, Clone)]
pub struct TableEdits {
    columns: u32,
    rows: u32,
    layout: Vec<Seg>,
    cells: HashMap<(RowRef, u32), String>,
    added: u32,
    log: Log<Step>,
}

impl TableEdits {
    /// No edits yet to a table of `rows` data rows and `columns` columns.
    #[must_use]
    pub fn new(rows: u32, columns: u32) -> Self {
        TableEdits {
            columns,
            rows,
            layout: Self::whole(rows),
            cells: HashMap::new(),
            added: 0,
            log: Log::default(),
        }
    }

    fn whole(rows: u32) -> Vec<Seg> {
        if rows == 0 {
            Vec::new()
        } else {
            vec![Seg::File { from: 0, len: rows }]
        }
    }

    /// How many rows there are now.
    #[must_use]
    pub fn rows(&self) -> u32 {
        self.layout
            .iter()
            .map(|seg| match seg {
                Seg::File { len, .. } => *len,
                Seg::New(_) => 1,
            })
            .fold(0_u32, u32::saturating_add)
    }

    /// The row at `position` among the rows as they are now.
    #[must_use]
    pub fn row_at(&self, position: u32) -> Option<RowRef> {
        let mut before = 0_u32;
        for seg in &self.layout {
            match seg {
                Seg::File { from, len } => {
                    if position < before.saturating_add(*len) {
                        return Some(RowRef::File(from + (position - before)));
                    }
                    before = before.saturating_add(*len);
                }
                Seg::New(id) => {
                    if position == before {
                        return Some(RowRef::New(*id));
                    }
                    before = before.saturating_add(1);
                }
            }
        }
        None
    }

    /// Where `row` is among the rows as they are now, when it has not been taken away.
    #[must_use]
    pub fn position_of(&self, row: RowRef) -> Option<u32> {
        let mut before = 0_u32;
        for seg in &self.layout {
            match (seg, row) {
                (Seg::File { from, len }, RowRef::File(at)) if at >= *from && at - from < *len => {
                    return Some(before + (at - from));
                }
                (Seg::New(id), RowRef::New(wanted)) if *id == wanted => return Some(before),
                (Seg::File { .. } | Seg::New(_), RowRef::File(_) | RowRef::New(_)) => {}
            }
            before = before.saturating_add(match seg {
                Seg::File { len, .. } => *len,
                Seg::New(_) => 1,
            });
        }
        None
    }

    /// Whether rows were added or taken away, so that the rows are no longer the file's own in
    /// the file's order.
    #[must_use]
    pub fn is_reshaped(&self) -> bool {
        self.layout != Self::whole(self.rows)
    }

    /// The text of the cell at `column` of `row`: the person's, else the table's own.
    #[must_use]
    pub fn cell(&self, table: &Table, row: RowRef, column: u32) -> String {
        match self.cells.get(&(row, column)) {
            Some(text) => text.clone(),
            None => Self::original(table, row, column),
        }
    }

    fn original(table: &Table, row: RowRef, column: u32) -> String {
        match row {
            RowRef::File(at) => table.cell(RowIndex(at), column as usize), // a u32 fits a usize
            RowRef::New(_) => String::new(),
        }
    }

    /// Change the cell at `column` of `row` to `text`. The same text as it has is no edit.
    pub fn set_cell(&mut self, table: &Table, row: RowRef, column: u32, text: &str) {
        if self.cell(table, row, column) == text {
            return;
        }
        let before = self.cells.get(&(row, column)).cloned();
        let after = (Self::original(table, row, column) != text).then(|| text.to_owned());
        let step = Step::Cell {
            row,
            column,
            before,
            after,
        };
        self.apply(&step, Way::Forward);
        self.log.push(step);
    }

    /// Add an empty row below the row at `position`; its place is returned.
    pub fn insert_below(&mut self, position: u32) -> u32 {
        let before = self.layout.clone();
        let at = self.cut(position.saturating_add(1));
        self.layout.insert(at, Seg::New(self.added));
        self.added += 1;
        let step = Step::Layout {
            before,
            after: self.layout.clone(),
        };
        self.log.push(step);
        position
            .saturating_add(1)
            .min(self.rows().saturating_sub(1))
    }

    /// Take away the rows at these places, in any order.
    pub fn delete(&mut self, positions: &[u32]) {
        let mut places: Vec<u32> = positions
            .iter()
            .copied()
            .filter(|at| *at < self.rows())
            .collect();
        places.sort_unstable();
        places.dedup();
        if places.is_empty() {
            return;
        }
        let before = self.layout.clone();
        // Runs of neighbouring places, the last first so the earlier ones stay where they are.
        let mut runs: Vec<Range<u32>> = Vec::new();
        for place in places {
            match runs.last_mut() {
                Some(run) if run.end == place => run.end += 1,
                Some(_) | None => runs.push(place..place + 1),
            }
        }
        for run in runs.into_iter().rev() {
            let from = self.cut(run.start);
            let to = self.cut(run.end);
            self.layout.drain(from..to);
        }
        let step = Step::Layout {
            before,
            after: self.layout.clone(),
        };
        self.log.push(step);
    }

    /// The index of the segment that starts at `position`, splitting a run of file rows there if
    /// the place falls inside one; the number of segments when `position` is the end.
    fn cut(&mut self, position: u32) -> usize {
        let mut before = 0_u32;
        let mut inside = None;
        for (index, seg) in self.layout.iter().enumerate() {
            let len = match seg {
                Seg::File { len, .. } => *len,
                Seg::New(_) => 1,
            };
            if position == before {
                return index;
            }
            if position < before.saturating_add(len) {
                inside = Some((index, position - before));
                break;
            }
            before = before.saturating_add(len);
        }
        let Some((index, head)) = inside else {
            return self.layout.len();
        };
        // An added row is one row long, so a place inside a segment is inside a run of file rows.
        if let Seg::File { from, len } = self.layout[index].clone() {
            self.layout[index] = Seg::File { from, len: head };
            self.layout.insert(
                index + 1,
                Seg::File {
                    from: from + head,
                    len: len - head,
                },
            );
            return index + 1;
        }
        self.layout.len()
    }

    fn apply(&mut self, step: &Step, way: Way) {
        match step {
            Step::Cell {
                row,
                column,
                before,
                after,
            } => {
                let value = match way {
                    Way::Forward => after,
                    Way::Backward => before,
                };
                match value {
                    Some(text) => {
                        self.cells.insert((*row, *column), text.clone());
                    }
                    None => {
                        self.cells.remove(&(*row, *column));
                    }
                }
            }
            Step::Layout { before, after } => {
                self.layout = match way {
                    Way::Forward => after.clone(),
                    Way::Backward => before.clone(),
                };
            }
        }
    }

    /// Take back the last edit. Whether there was one.
    pub fn undo(&mut self) -> Undid {
        let Some(step) = self.log.undo().cloned() else {
            return Undid::Nothing;
        };
        self.apply(&step, Way::Backward);
        Undid::Something
    }

    /// Do the edit taken back again. Whether there was one.
    pub fn redo(&mut self) -> Undid {
        let Some(step) = self.log.redo().cloned() else {
            return Undid::Nothing;
        };
        self.apply(&step, Way::Forward);
        Undid::Something
    }

    /// How many edits can be taken back.
    #[must_use]
    pub fn undoable(&self) -> usize {
        self.log.undoable()
    }

    /// How many edits can be done again.
    #[must_use]
    pub fn redoable(&self) -> usize {
        self.log.redoable()
    }

    /// The state of the edits now.
    #[must_use]
    pub fn stamp(&self) -> EditStamp {
        self.log.stamp()
    }

    /// Whether the table has changes that are not saved.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.log.is_modified()
    }

    /// The table as of `stamp` is on disk.
    pub fn mark_saved(&mut self, stamp: EditStamp) {
        self.log.mark_saved(stamp);
    }

    /// How many columns the table has.
    #[must_use]
    pub fn columns(&self) -> u32 {
        self.columns
    }

    /// What writing does, run of rows by run of rows.
    pub(super) fn ops(&self) -> Vec<Op> {
        let mut changed: BTreeMap<u32, BTreeMap<u32, String>> = BTreeMap::new();
        for ((row, column), text) in &self.cells {
            if let RowRef::File(at) = row {
                changed
                    .entry(*at)
                    .or_default()
                    .insert(*column, text.clone());
            }
        }
        let mut ops = Vec::new();
        for seg in &self.layout {
            match seg {
                Seg::File { from, len } => {
                    let end = from + len;
                    let mut at = *from;
                    for (row, cells) in changed.range(*from..end) {
                        if *row > at {
                            ops.push(Op::Keep(at..*row));
                        }
                        ops.push(Op::Edit(*row, cells.clone()));
                        at = row + 1;
                    }
                    if at < end {
                        ops.push(Op::Keep(at..end));
                    }
                }
                Seg::New(id) => {
                    let cells = (0..self.columns)
                        .map(|column| {
                            self.cells
                                .get(&(RowRef::New(*id), column))
                                .cloned()
                                .unwrap_or_default()
                        })
                        .collect();
                    ops.push(Op::Insert(cells));
                }
            }
        }
        ops
    }
}

/// Which way a step is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    Forward,
    Backward,
}

/// Whether an undo or a redo had an edit to take back or do again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undid {
    /// There was one, and it is done.
    Something,
    /// There was none.
    Nothing,
}
