use super::column::ColumnSort;
use super::model::{Look, SheetNo, SheetTotal, TableIn, TableOut, TableParams, TableStage};
use crate::stage::find::{FindHits, FindOut, HitStep, StageFind};
use crate::stage::row::RowNo;
use crate::stage::zoom::ContentScale;
use crate::typed::TypedText;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (TableStage, Vec<TableOut>);

impl Machine for TableStage {
    type In = TableIn;
    type Out = TableOut;
    type Params = TableParams;
    type Ctx = ();

    fn step(self, input: TableIn, _at: Stamp, params: &TableParams, _cx: &()) -> Step {
        let look = self.look();
        let (sheet, row, sort) = (self.sheet(), self.row(), self.sort());
        let (sheet, row, sort, look, outs) = advance((sheet, row, sort), look, input, params);
        (TableStage::of(sheet, row, sort, look), outs)
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
        | TableIn::Move(_)
        | TableIn::PressHeader(_)
        | TableIn::Deselect
        | TableIn::Find(_)
        | TableIn::Results { .. }
        | TableIn::NextHit
        | TableIn::PreviousHit
        | TableIn::GoToHit(_)
        | TableIn::CloseFind
        | TableIn::ZoomStep(_)
        | TableIn::ZoomActual
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

/// The sheet, the cursor, the sort, the find and the zoom after `input`, and what it asks for.
fn advance(
    (sheet, row, sort): (SheetNo, Option<RowNo>, Option<ColumnSort>),
    look: Look,
    input: TableIn,
    params: &TableParams,
) -> (
    SheetNo,
    Option<RowNo>,
    Option<ColumnSort>,
    Look,
    Vec<TableOut>,
) {
    let Look { find, scale } = look;
    let stay = |find: Option<StageFind>, outs: Vec<TableOut>| {
        (sheet, row, sort, Look { find, scale }, outs)
    };
    match input {
        // Another sheet has other rows and other columns: the cursor and the sort do not carry
        // over, and a find that is up searches this sheet.
        TableIn::NextSheet | TableIn::PreviousSheet | TableIn::ChooseSheet(_) => {
            match sheet_asked(sheet, &input, params.sheets).filter(|next| *next != sheet) {
                Some(next) => {
                    let (find, outs) = asked_again(find);
                    (next, None, None, Look { find, scale }, outs)
                }
                None => stay(find, vec![]),
            }
        }
        TableIn::Select(picked) => (sheet, Some(picked), sort, Look { find, scale }, vec![]),
        TableIn::Move(step) => (
            sheet,
            step.from(row, params.rows, params.page).or(row),
            sort,
            Look { find, scale },
            vec![],
        ),
        // The rows change places, so the row the cursor was on is somewhere else.
        TableIn::PressHeader(column) if column < params.columns => (
            sheet,
            None,
            ColumnSort::after_press(sort, column),
            Look { find, scale },
            vec![],
        ),
        TableIn::PressHeader(_) | TableIn::Elapsed => stay(find, vec![]),
        TableIn::Deselect => (sheet, None, sort, Look { find, scale }, vec![]),
        TableIn::Find(query) => {
            let outs = vec![TableOut::Find(FindOut::asked(&query))];
            stay(Some(StageFind::asked(query)), outs)
        }
        TableIn::Results {
            query,
            count,
            nearest,
        } => {
            let (find, outs) = answered(find, &query, FindHits::answered(count, nearest));
            stay(find, outs)
        }
        TableIn::NextHit => {
            let (find, outs) = hit_moved(find, |hits| hits.stepped(HitStep::Next));
            stay(find, outs)
        }
        TableIn::PreviousHit => {
            let (find, outs) = hit_moved(find, |hits| hits.stepped(HitStep::Previous));
            stay(find, outs)
        }
        TableIn::GoToHit(hit) => {
            let (find, outs) = hit_moved(find, |hits| hits.jumped(hit));
            stay(find, outs)
        }
        TableIn::CloseFind => {
            let outs = match find {
                Some(_) => vec![TableOut::Find(FindOut::Clear)],
                None => vec![],
            };
            stay(None, outs)
        }
        TableIn::ZoomStep(dir) => (
            sheet,
            row,
            sort,
            Look {
                find,
                scale: scale.stepped(dir),
            },
            vec![],
        ),
        TableIn::ZoomActual => (
            sheet,
            row,
            sort,
            Look {
                find,
                scale: ContentScale::ACTUAL,
            },
            vec![],
        ),
    }
}
