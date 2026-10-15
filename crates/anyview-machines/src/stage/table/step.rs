use super::cells;
use super::column::ColumnSort;
use super::model::{Cells, Look, SheetNo, SheetTotal, TableIn, TableOut, TableParams, TableStage};
use crate::stage::data_edits::RowOrder;
use crate::stage::find::{FindHits, FindOut, HitStep, StageFind};
use crate::stage::row::RowNo;
use crate::stage::zoom::ContentScale;
use crate::typed::TypedText;
use ds_core::machine::Machine;
use ds_core::selection::{Pick, RowSelection};
use ds_core::time::stamp::Stamp;

type Step = (TableStage, Vec<TableOut>);

impl Machine for TableStage {
    type In = TableIn;
    type Out = TableOut;
    type Params = TableParams;
    type Ctx = ();

    fn step(self, input: TableIn, _at: Stamp, params: &TableParams, _cx: &()) -> Step {
        let look = self.look();
        let (sheet, sort) = (self.sheet(), self.sort());
        let rows = self.selection().cloned().unwrap_or_default();
        let (sheet, rows, sort, look, outs) = advance((sheet, rows, sort), look, input, params);
        (TableStage::of(sheet, rows, sort, look), outs)
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TableStage::Browsing { .. } | TableStage::Selected { .. } => None,
        }
    }
}

/// The sheet `input` asks for from `sheet`, or `None` when it asks for none or for one the file
/// does not have.
fn sheet_asked(sheet: SheetNo, input: &TableIn, total: SheetTotal) -> Option<SheetNo> {
    let last = total.0.checked_sub(1)?;
    match input {
        TableIn::NextSheet => Some(SheetNo(sheet.0.saturating_add(1).min(last))),
        TableIn::PreviousSheet => Some(SheetNo(sheet.0.saturating_sub(1).min(last))),
        TableIn::ChooseSheet(chosen) if chosen.0 <= last => Some(*chosen),
        TableIn::ChooseSheet(_)
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
        | TableIn::Typing(_)
        | TableIn::EditAt(..)
        | TableIn::FocusCell(..)
        | TableIn::StepCell(_)
        | TableIn::AddRow
        | TableIn::DeleteRows
        | TableIn::Session(_)
        | TableIn::Elapsed => None,
    }
}

/// The outputs that show the current hit of `hits`, if there is one.
fn shown(hits: FindHits) -> Vec<TableOut> {
    hits.current()
        .map(|hit| TableOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect()
}

/// The find after `moved` changed which hit is current, and the output that shows the hit it
/// lands on; with no find up, nothing moves.
fn hit_moved(
    find: Option<StageFind>,
    moved: impl FnOnce(FindHits) -> FindHits,
) -> (Option<StageFind>, Vec<TableOut>) {
    match find {
        Some(StageFind { query, hits }) => {
            let hits = moved(hits);
            (Some(StageFind { query, hits }), shown(hits))
        }
        None => (None, vec![]),
    }
}

/// A find that was up over another sheet, asked again of this one.
fn asked_again(find: Option<StageFind>) -> (Option<StageFind>, Vec<TableOut>) {
    match find {
        Some(StageFind { query, .. }) if !query.is_empty() => {
            let outs = vec![TableOut::Find(FindOut::asked(&query))];
            (Some(StageFind::asked(query)), outs)
        }
        Some(_) | None => (find, vec![]),
    }
}

/// The find after the search answered `answered` with its hits: only the search for the text now
/// up is heard, and the reader is taken to the hit nearest where they are.
fn answered(
    find: Option<StageFind>,
    asked: &TypedText,
    hits: FindHits,
) -> (Option<StageFind>, Vec<TableOut>) {
    match find {
        Some(StageFind { query, .. }) if query == *asked => {
            (Some(StageFind { query, hits }), shown(hits))
        }
        other => (other, vec![]),
    }
}

/// The cursor of `rows` as the stage names a row.
fn cursor_of(rows: &RowSelection) -> Option<RowNo> {
    let at = rows.cursor().or_else(|| rows.rows().next())?;
    u32::try_from(at).ok().map(RowNo)
}

/// `rows` after `pick`, when the sheet has the row of `pick`.
fn picked(rows: RowSelection, pick: Pick, params: &TableParams) -> RowSelection {
    match pick.row < params.rows as usize {
        // a u32 fits a usize
        true => rows.picked(pick),
        false => rows,
    }
}

/// The picked rows after Esc: several narrow to the cursor's row, one is put away.
fn escaped(rows: RowSelection) -> RowSelection {
    match rows.count() > 1 {
        true => rows.narrowed(),
        false => RowSelection::none(),
    }
}

/// The sheet, the picked rows, the sort, the find and the zoom after `input`, and what it asks for.
fn advance(
    (sheet, rows, sort): (SheetNo, RowSelection, Option<ColumnSort>),
    look: Look,
    input: TableIn,
    params: &TableParams,
) -> (
    SheetNo,
    RowSelection,
    Option<ColumnSort>,
    Look,
    Vec<TableOut>,
) {
    let Look { find, scale, cells } = look;
    let stay = |rows: RowSelection, find: Option<StageFind>, outs: Vec<TableOut>| {
        (
            sheet,
            rows,
            sort,
            Look {
                find,
                scale,
                cells: cells.clone(),
            },
            outs,
        )
    };
    match input {
        // Another sheet has other rows and other columns: the picked rows and the sort do not
        // carry over, and a find that is up searches this sheet.
        TableIn::NextSheet | TableIn::PreviousSheet | TableIn::ChooseSheet(_) => {
            match sheet_asked(sheet, &input, params.sheets).filter(|next| *next != sheet) {
                Some(next) => {
                    let (find, outs) = asked_again(find);
                    let look = Look {
                        find,
                        scale,
                        cells: Cells::default(),
                    };
                    (next, RowSelection::none(), None, look, outs)
                }
                None => stay(rows, find, vec![]),
            }
        }
        TableIn::Select(row) => stay(RowSelection::single(row.0 as usize), find, vec![]), // a u32 fits a usize
        TableIn::Pick(pick) => stay(picked(rows, pick, params), find, vec![]),
        TableIn::SelectAll => stay(rows.all(params.rows as usize), find, vec![]), // a u32 fits a usize
        TableIn::Move(step) => {
            let moved = step.from(cursor_of(&rows), params.rows, params.page);
            let rows = moved.map_or(rows, |to| RowSelection::single(to.0 as usize)); // a u32 fits a usize
            stay(rows, find, vec![])
        }
        // The rows change places: the picked ones stay at their places until the view, which
        // has the new order, says where they went.
        // Once rows were added or taken away the order a sort gives no longer fits the rows.
        TableIn::PressHeader(column)
            if column < params.columns && cells.edits.log.rows == RowOrder::AsFile =>
        {
            (
                sheet,
                rows,
                ColumnSort::after_press(sort, column),
                Look { find, scale, cells },
                vec![],
            )
        }
        TableIn::PressHeader(_) | TableIn::Elapsed => stay(rows, find, vec![]),
        TableIn::Typing(_)
        | TableIn::EditAt(..)
        | TableIn::FocusCell(..)
        | TableIn::StepCell(_)
        | TableIn::AddRow
        | TableIn::DeleteRows
        | TableIn::Session(_) => {
            let worked = cells::apply((rows, sort, cells), input, params);
            (
                sheet,
                worked.rows,
                worked.sort,
                Look {
                    find,
                    scale,
                    cells: worked.cells,
                },
                worked.outs,
            )
        }
        TableIn::Reseat(moved) => match rows.is_empty() {
            true => stay(rows, find, vec![]),
            false => stay(moved, find, vec![]),
        },
        TableIn::Deselect => stay(escaped(rows), find, vec![]),
        TableIn::Copy => {
            let outs = match rows.is_empty() {
                true => vec![],
                false => vec![TableOut::Copy(rows.clone())],
            };
            stay(rows, find, outs)
        }
        TableIn::Find(query) => {
            let outs = vec![TableOut::Find(FindOut::asked(&query))];
            stay(rows, Some(StageFind::asked(query)), outs)
        }
        TableIn::Results {
            query,
            count,
            nearest,
        } => {
            let (find, outs) = answered(find, &query, FindHits::answered(count, nearest));
            stay(rows, find, outs)
        }
        TableIn::NextHit => {
            let (find, outs) = hit_moved(find, |hits| hits.stepped(HitStep::Next));
            stay(rows, find, outs)
        }
        TableIn::PreviousHit => {
            let (find, outs) = hit_moved(find, |hits| hits.stepped(HitStep::Previous));
            stay(rows, find, outs)
        }
        TableIn::GoToHit(hit) => {
            let (find, outs) = hit_moved(find, |hits| hits.jumped(hit));
            stay(rows, find, outs)
        }
        TableIn::CloseFind => {
            let outs = match find {
                Some(_) => vec![TableOut::Find(FindOut::Clear)],
                None => vec![],
            };
            stay(rows, None, outs)
        }
        TableIn::ZoomStep(dir) => (
            sheet,
            rows,
            sort,
            Look {
                find,
                scale: scale.stepped(dir),
                cells,
            },
            vec![],
        ),
        TableIn::ZoomActual => (
            sheet,
            rows,
            sort,
            Look {
                find,
                scale: ContentScale::ACTUAL,
                cells,
            },
            vec![],
        ),
    }
}
