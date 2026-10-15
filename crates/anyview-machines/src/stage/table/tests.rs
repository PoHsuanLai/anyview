use super::*;
use crate::edits::Rewind;
use crate::stage::data_edits::{
    AfterTyping, DataEdits, EditDepth, EditLog, EditSessionIn, EditSessionOut, RowOrder, TypingIn,
    TypingSeed, TypingState,
};
use crate::stage::find::{FindHits, FindOut, HitCount, HitIndex, StageFind};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::text::{Changes, Editable};
use crate::stage::zoom::{ContentScale, ZoomDir};
use crate::typed::TypedText;
use ds_core::machine::Machine;
use ds_core::selection::{Pick, PickHow, RowSelection};
use ds_core::time::stamp::Stamp;

const fn browsing(sheet: u32) -> TableStage {
    browsing_by(sheet, None)
}
fn selected(sheet: u32, row: u32) -> TableStage {
    selected_by(sheet, row, None)
}
const fn browsing_by(sheet: u32, sort: Option<ColumnSort>) -> TableStage {
    TableStage::Browsing {
        sheet: SheetNo(sheet),
        sort,
        find: None,
        scale: ContentScale::ACTUAL,
        cells: Cells::NONE,
    }
}
fn selected_by(sheet: u32, row: u32, sort: Option<ColumnSort>) -> TableStage {
    TableStage::Selected {
        sheet: SheetNo(sheet),
        rows: RowSelection::single(row as usize),
        sort,
        find: None,
        scale: ContentScale::ACTUAL,
        cells: Cells::NONE,
    }
}
/// Several rows picked, with the anchor and the cursor where the last pick left them.
fn several(
    sheet: u32,
    (anchor, cursor): (usize, usize),
    rows: impl IntoIterator<Item = usize>,
    sort: Option<ColumnSort>,
) -> TableStage {
    TableStage::Selected {
        sheet: SheetNo(sheet),
        rows: RowSelection::of_rows(Some(anchor), Some(cursor), rows),
        sort,
        find: None,
        scale: ContentScale::ACTUAL,
        cells: Cells::NONE,
    }
}
const fn pick(row: usize, how: PickHow) -> TableIn {
    TableIn::Pick(Pick { row, how })
}
const fn up(column: u32) -> Option<ColumnSort> {
    Some(ColumnSort {
        column,
        direction: SortDirection::Ascending,
    })
}
const fn down(column: u32) -> Option<ColumnSort> {
    Some(ColumnSort {
        column,
        direction: SortDirection::Descending,
    })
}

/// Name, sheets in the file, state before, input, state after.
type Case = (&'static str, u32, TableStage, TableIn, TableStage);

fn cases() -> Vec<Case> {
    vec![
        (
            "next sheet moves on",
            3,
            browsing(0),
            TableIn::NextSheet,
            browsing(1),
        ),
        (
            "next sheet stops at the last",
            3,
            browsing(2),
            TableIn::NextSheet,
            browsing(2),
        ),
        (
            "previous sheet moves back",
            3,
            browsing(2),
            TableIn::PreviousSheet,
            browsing(1),
        ),
        (
            "previous sheet stops at the first",
            3,
            browsing(0),
            TableIn::PreviousSheet,
            browsing(0),
        ),
        (
            "choosing a sheet the file has",
            3,
            browsing(0),
            TableIn::ChooseSheet(SheetNo(2)),
            browsing(2),
        ),
        (
            "choosing a sheet it has not is ignored",
            3,
            browsing(1),
            TableIn::ChooseSheet(SheetNo(3)),
            browsing(1),
        ),
        (
            "a file with no sheets stays",
            0,
            browsing(0),
            TableIn::NextSheet,
            browsing(0),
        ),
        (
            "selecting a row",
            1,
            browsing(0),
            TableIn::Select(RowNo(4)),
            selected(0, 4),
        ),
        (
            "selecting another row",
            1,
            selected(0, 4),
            TableIn::Select(RowNo(9)),
            selected(0, 9),
        ),
        (
            "deselecting",
            1,
            selected(0, 4),
            TableIn::Deselect,
            browsing(0),
        ),
        (
            "deselecting with no row",
            1,
            browsing(0),
            TableIn::Deselect,
            browsing(0),
        ),
        (
            "another sheet drops the cursor",
            2,
            selected(0, 4),
            TableIn::NextSheet,
            browsing(1),
        ),
        (
            "no other sheet keeps the cursor",
            2,
            selected(1, 4),
            TableIn::NextSheet,
            selected(1, 4),
        ),
        (
            "choosing the same sheet keeps the cursor",
            2,
            selected(1, 4),
            TableIn::ChooseSheet(SheetNo(1)),
            selected(1, 4),
        ),
        (
            "the clock changes nothing",
            1,
            selected(0, 4),
            TableIn::Elapsed,
            selected(0, 4),
        ),
        (
            "down with no cursor picks the first row",
            1,
            browsing(0),
            TableIn::Move(RowStep::Down),
            selected(0, 0),
        ),
        (
            "down moves the cursor one row",
            1,
            selected(0, 4),
            TableIn::Move(RowStep::Down),
            selected(0, 5),
        ),
        (
            "page down moves by the room",
            1,
            selected(0, 4),
            TableIn::Move(RowStep::PageDown),
            selected(0, 8),
        ),
        (
            "end goes to the last row",
            1,
            selected(0, 4),
            TableIn::Move(RowStep::Bottom),
            selected(0, 9),
        ),
        (
            "home goes to the first row",
            1,
            selected(0, 4),
            TableIn::Move(RowStep::Top),
            selected(0, 0),
        ),
        (
            "a header press sorts ascending",
            1,
            browsing(0),
            TableIn::PressHeader(1),
            browsing_by(0, up(1)),
        ),
        (
            "the same header again sorts descending",
            1,
            browsing_by(0, up(1)),
            TableIn::PressHeader(1),
            browsing_by(0, down(1)),
        ),
        (
            "a third press puts the sort away",
            1,
            browsing_by(0, down(1)),
            TableIn::PressHeader(1),
            browsing(0),
        ),
        (
            "another header sorts that column ascending",
            1,
            browsing_by(0, down(1)),
            TableIn::PressHeader(2),
            browsing_by(0, up(2)),
        ),
        (
            "a sort keeps the picked places until the view says where the rows went",
            1,
            several(0, (4, 7), 4..8, None),
            TableIn::PressHeader(0),
            several(0, (4, 7), 4..8, up(0)),
        ),
        (
            "the view says where the sort put the picked rows",
            1,
            several(0, (4, 7), 4..8, up(0)),
            TableIn::Reseat(RowSelection::of_rows(Some(2), Some(9), [2, 9])),
            several(0, (2, 9), [2, 9], up(0)),
        ),
        (
            "a row put elsewhere with nothing picked is not heard",
            1,
            browsing_by(0, up(1)),
            TableIn::Reseat(RowSelection::single(3)),
            browsing_by(0, up(1)),
        ),
        (
            "a header the sheet has not is ignored",
            1,
            selected_by(0, 4, up(1)),
            TableIn::PressHeader(3),
            selected_by(0, 4, up(1)),
        ),
        (
            "the cursor moves inside the sort",
            1,
            selected_by(0, 4, down(1)),
            TableIn::Move(RowStep::Down),
            selected_by(0, 5, down(1)),
        ),
        (
            "putting the cursor away keeps the sort",
            1,
            selected_by(0, 4, down(1)),
            TableIn::Deselect,
            browsing_by(0, down(1)),
        ),
        (
            "another sheet has other columns, so the sort goes",
            2,
            selected_by(0, 4, up(1)),
            TableIn::NextSheet,
            browsing(1),
        ),
        (
            "no other sheet keeps the sort",
            2,
            browsing_by(1, up(1)),
            TableIn::NextSheet,
            browsing_by(1, up(1)),
        ),
        (
            "a plain pick replaces the picked rows",
            1,
            several(0, (4, 7), 4..8, None),
            pick(2, PickHow::Replace),
            selected(0, 2),
        ),
        (
            "a pick with none picked picks the row",
            1,
            browsing(0),
            pick(4, PickHow::Replace),
            selected(0, 4),
        ),
        (
            "a toggle adds a row apart",
            1,
            selected(0, 4),
            pick(7, PickHow::Toggle),
            several(0, (7, 7), [4, 7], None),
        ),
        (
            "a toggle takes a picked row out",
            1,
            several(0, (4, 7), 4..8, None),
            pick(5, PickHow::Toggle),
            several(0, (5, 5), [4, 6, 7], None),
        ),
        (
            "toggling the last picked row puts the selection away",
            1,
            selected(0, 4),
            pick(4, PickHow::Toggle),
            browsing(0),
        ),
        (
            "an extend picks the rows from the anchor",
            1,
            selected(0, 4),
            pick(7, PickHow::Extend),
            several(0, (4, 7), 4..8, None),
        ),
        (
            "an extend upwards from the anchor",
            1,
            selected(0, 6),
            pick(3, PickHow::Extend),
            several(0, (6, 3), 3..7, None),
        ),
        (
            "an extend again measures from the same anchor",
            1,
            several(0, (4, 8), 4..9, None),
            pick(5, PickHow::Extend),
            several(0, (4, 5), 4..6, None),
        ),
        (
            "a pick of a row the sheet has not is ignored",
            1,
            selected(0, 4),
            pick(10, PickHow::Replace),
            selected(0, 4),
        ),
        (
            "select all picks every row and keeps the cursor",
            1,
            selected(0, 4),
            TableIn::SelectAll,
            several(0, (4, 4), 0..10, None),
        ),
        (
            "select all with nothing picked picks them from the first",
            1,
            browsing(0),
            TableIn::SelectAll,
            several(0, (0, 0), 0..10, None),
        ),
        (
            "a move narrows the picked rows to the row it reaches",
            1,
            several(0, (4, 7), 4..8, None),
            TableIn::Move(RowStep::Down),
            selected(0, 8),
        ),
        (
            "Esc narrows several picked rows to the cursor's",
            1,
            several(0, (4, 7), 4..8, None),
            TableIn::Deselect,
            selected(0, 7),
        ),
        (
            "Esc again puts the row away",
            1,
            selected(0, 7),
            TableIn::Deselect,
            browsing(0),
        ),
        (
            "a find landing on a hit picks just its row",
            1,
            several(0, (4, 7), 4..8, None),
            TableIn::Select(RowNo(2)),
            selected(0, 2),
        ),
        (
            "another sheet drops every picked row",
            2,
            several(0, (4, 7), 4..8, None),
            TableIn::NextSheet,
            browsing(1),
        ),
        (
            "the sort stays with the rows picked when the same sheet is chosen",
            2,
            several(1, (4, 7), 4..8, up(1)),
            TableIn::ChooseSheet(SheetNo(1)),
            several(1, (4, 7), 4..8, up(1)),
        ),
    ]
}

#[test]
fn the_table_stage_steps_as_the_table_says() {
    for (name, sheets, before, input, after) in cases() {
        let params = TableParams {
            sheets: SheetTotal(sheets),
            columns: 3,
            rows: 10,
            page: 4,
            editable: Editable::No,
        };
        let (next, outs) = before.step(input, Stamp(0), &params, &());
        assert_eq!(next, after, "{name}");
        assert!(outs.is_empty(), "{name}");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn a_stage_knows_its_sheet_and_row() {
    assert_eq!(selected(2, 7).sheet(), SheetNo(2));
    assert_eq!(selected(2, 7).row(), Some(RowNo(7)));
    assert_eq!(browsing(1).row(), None);
    assert_eq!(selected_by(2, 7, down(1)).sort(), down(1));
    // The cursor is the row picked last; the count is every row picked.
    let picked = several(0, (2, 9), [2, 3, 9], None);
    assert_eq!(picked.row(), Some(RowNo(9)));
    assert_eq!(picked.selected_count(), 3);
    assert_eq!(browsing(0).selected_count(), 0);
    assert_eq!(browsing(0).selection(), None);
}

#[test]
fn copy_asks_for_the_picked_rows_and_with_none_picked_asks_for_nothing() {
    let params = TableParams {
        sheets: SheetTotal(1),
        columns: 3,
        rows: 10,
        page: 4,
        editable: Editable::No,
    };
    let picked = several(0, (2, 4), 2..5, None);
    let (next, outs) = picked.clone().step(TableIn::Copy, Stamp(0), &params, &());
    assert_eq!(next, picked, "copying changes nothing");
    assert_eq!(
        outs,
        vec![TableOut::Copy(RowSelection::of_rows(
            Some(2),
            Some(4),
            2..5
        ))]
    );
    let (next, outs) = browsing(0).step(TableIn::Copy, Stamp(0), &params, &());
    assert_eq!(next, browsing(0));
    assert!(outs.is_empty());
}

const CAT: TypedText = TypedText::from_static("cat");

fn finding(sheet: u32, hits: FindHits) -> TableStage {
    TableStage::Browsing {
        sheet: SheetNo(sheet),
        sort: None,
        find: Some(StageFind { query: CAT, hits }),
        scale: ContentScale::ACTUAL,
        cells: Cells::NONE,
    }
}

fn found(count: u32, at: u32) -> FindHits {
    FindHits::answered(HitCount(count), HitIndex(at))
}

fn zoomed(stage: TableStage, scale: ContentScale) -> TableStage {
    match stage {
        TableStage::Browsing {
            sheet,
            sort,
            find,
            cells,
            ..
        } => TableStage::Browsing {
            sheet,
            sort,
            find,
            scale,
            cells,
        },
        TableStage::Selected {
            sheet,
            rows,
            sort,
            find,
            cells,
            ..
        } => TableStage::Selected {
            sheet,
            rows,
            sort,
            find,
            scale,
            cells,
        },
    }
}

/// Name, state before, input, state after, outputs.
type FindCase = (&'static str, TableStage, TableIn, TableStage, Vec<TableOut>);

fn search(find: FindOut) -> Vec<TableOut> {
    vec![TableOut::Find(find)]
}

#[test]
fn a_find_in_a_table_asks_for_a_search_and_follows_its_answer() {
    let cases: Vec<FindCase> = vec![
        (
            "find asks for a search",
            browsing(0),
            TableIn::Find(CAT),
            finding(0, FindHits::Pending),
            search(FindOut::Search(CAT)),
        ),
        (
            "find with nothing typed puts the marks away",
            browsing(0),
            TableIn::Find(TypedText::EMPTY),
            TableStage::Browsing {
                sheet: SheetNo(0),
                sort: None,
                find: Some(StageFind {
                    query: TypedText::EMPTY,
                    hits: FindHits::Idle,
                }),
                scale: ContentScale::ACTUAL,
                cells: Cells::NONE,
            },
            search(FindOut::Clear),
        ),
        (
            "the answer shows the nearest hit",
            finding(0, FindHits::Pending),
            TableIn::Results {
                query: CAT,
                count: HitCount(3),
                nearest: HitIndex(1),
            },
            finding(0, found(3, 1)),
            search(FindOut::ShowHit(HitIndex(1))),
        ),
        (
            "the answer to an earlier text is dropped",
            finding(0, FindHits::Pending),
            TableIn::Results {
                query: TypedText::from_static("dog"),
                count: HitCount(3),
                nearest: HitIndex(0),
            },
            finding(0, FindHits::Pending),
            vec![],
        ),
        (
            "the answer with no hits shows none",
            finding(0, FindHits::Pending),
            TableIn::Results {
                query: CAT,
                count: HitCount(0),
                nearest: HitIndex(0),
            },
            finding(0, FindHits::NoMatch),
            vec![],
        ),
        (
            "next hit moves on and shows it",
            finding(0, found(3, 1)),
            TableIn::NextHit,
            finding(0, found(3, 2)),
            search(FindOut::ShowHit(HitIndex(2))),
        ),
        (
            "next hit wraps to the first",
            finding(0, found(3, 2)),
            TableIn::NextHit,
            finding(0, found(3, 0)),
            search(FindOut::ShowHit(HitIndex(0))),
        ),
        (
            "previous hit wraps to the last",
            finding(0, found(3, 0)),
            TableIn::PreviousHit,
            finding(0, found(3, 2)),
            search(FindOut::ShowHit(HitIndex(2))),
        ),
        (
            "going to a hit shows it",
            finding(0, found(3, 0)),
            TableIn::GoToHit(HitIndex(2)),
            finding(0, found(3, 2)),
            search(FindOut::ShowHit(HitIndex(2))),
        ),
        (
            "next hit with no find up does nothing",
            browsing(0),
            TableIn::NextHit,
            browsing(0),
            vec![],
        ),
        (
            "closing the find puts the marks away",
            finding(0, found(3, 1)),
            TableIn::CloseFind,
            browsing(0),
            search(FindOut::Clear),
        ),
        (
            "the cursor moves with the find up",
            finding(0, found(3, 1)),
            TableIn::Select(RowNo(4)),
            TableStage::Selected {
                sheet: SheetNo(0),
                rows: RowSelection::single(4),
                sort: None,
                find: Some(StageFind {
                    query: CAT,
                    hits: found(3, 1),
                }),
                scale: ContentScale::ACTUAL,
                cells: Cells::NONE,
            },
            vec![],
        ),
        (
            "another sheet is searched again",
            finding(0, found(3, 1)),
            TableIn::NextSheet,
            finding(1, FindHits::Pending),
            search(FindOut::Search(CAT)),
        ),
    ];
    for (name, before, input, after, want) in cases {
        let params = TableParams {
            sheets: SheetTotal(2),
            columns: 3,
            rows: 10,
            page: 4,
            editable: Editable::No,
        };
        let (next, outs) = before.step(input, Stamp(0), &params, &());
        assert_eq!(next, after, "{name}: state");
        assert_eq!(outs, want, "{name}: outputs");
    }
}

#[test]
fn zooming_a_table_steps_its_scale_and_keeps_the_cursor() {
    let bigger = ContentScale::ACTUAL.stepped(ZoomDir::In);
    let smaller = ContentScale::ACTUAL.stepped(ZoomDir::Out);
    // name, state before, input, state after
    let cases: Vec<(&str, TableStage, TableIn, TableStage)> = vec![
        (
            "in",
            selected(0, 4),
            TableIn::ZoomStep(ZoomDir::In),
            zoomed(selected(0, 4), bigger),
        ),
        (
            "out",
            browsing(0),
            TableIn::ZoomStep(ZoomDir::Out),
            zoomed(browsing(0), smaller),
        ),
        (
            "actual size",
            zoomed(selected(0, 4), bigger),
            TableIn::ZoomActual,
            selected(0, 4),
        ),
    ];
    for (name, before, input, after) in cases {
        let (next, outs) = before.step(input, Stamp(0), &TableParams::default(), &());
        assert_eq!(next, after, "{name}");
        assert!(outs.is_empty(), "{name}");
    }
}

const EDITABLE: TableParams = TableParams {
    sheets: SheetTotal(1),
    columns: 3,
    rows: 10,
    page: 4,
    editable: Editable::Yes,
};

/// A row picked, with the cell cursor, what is typed and what the window says of the edits.
fn at_cell(row: u32, column: Option<u32>, typing: TypingState, log: EditLog) -> TableStage {
    TableStage::Selected {
        sheet: SheetNo(0),
        rows: RowSelection::single(row as usize),
        sort: None,
        find: None,
        scale: ContentScale::ACTUAL,
        cells: Cells {
            focus: column.map_or(CellFocus::Row, |column| CellFocus::Cell(ColumnNo(column))),
            edits: DataEdits { log, typing },
        },
    }
}

fn unsaved(undo: u32) -> EditLog {
    EditLog {
        changes: Changes::Unsaved,
        undo: EditDepth(undo),
        ..EditLog::NONE
    }
}

fn typed(text: &'static str) -> TypedText {
    TypedText::from_static(text)
}

/// Name, whether the file takes edits, state before, input, state after, outputs.
type CellCase = (
    &'static str,
    Editable,
    TableStage,
    TableIn,
    TableStage,
    Vec<TableOut>,
);

#[test]
fn cells_are_stepped_typed_into_and_taken_as_a_spreadsheet_does() {
    let none = EditLog::NONE;
    let begin = |seed| TableIn::Typing(TypingIn::Begin(seed));
    let commit = |text, then| TableIn::Typing(TypingIn::Commit(text, then));
    let sets = |row, column, text| {
        vec![TableOut::Set {
            row: RowNo(row),
            column: ColumnNo(column),
            text,
            sort: None,
        }]
    };
    let cases: Vec<CellCase> = vec![
        (
            "return types into the first cell of the picked row",
            Editable::Yes,
            selected(0, 2),
            begin(TypingSeed::Existing),
            at_cell(2, Some(0), TypingState::Value(TypingSeed::Existing), none),
            vec![],
        ),
        (
            "return types into the cell the cursor is on",
            Editable::Yes,
            at_cell(2, Some(1), TypingState::Idle, none),
            begin(TypingSeed::Existing),
            at_cell(2, Some(1), TypingState::Value(TypingSeed::Existing), none),
            vec![],
        ),
        (
            "a key typed on a picked cell replaces its text",
            Editable::Yes,
            at_cell(2, Some(1), TypingState::Idle, none),
            begin(TypingSeed::Typed(typed("x"))),
            at_cell(
                2,
                Some(1),
                TypingState::Value(TypingSeed::Typed(typed("x"))),
                none,
            ),
            vec![],
        ),
        (
            "with no row picked there is no cell to type into",
            Editable::Yes,
            browsing(0),
            begin(TypingSeed::Existing),
            browsing(0),
            vec![],
        ),
        (
            "a file that cannot be edited starts no typing",
            Editable::No,
            selected(0, 2),
            begin(TypingSeed::Existing),
            selected(0, 2),
            vec![],
        ),
        (
            "a double-click picks the row alone and types into the cell",
            Editable::Yes,
            browsing(0),
            TableIn::EditAt(RowNo(4), ColumnNo(2)),
            at_cell(4, Some(2), TypingState::Value(TypingSeed::Existing), none),
            vec![],
        ),
        (
            "a double-click below the last row does nothing",
            Editable::Yes,
            browsing(0),
            TableIn::EditAt(RowNo(40), ColumnNo(0)),
            browsing(0),
            vec![],
        ),
        (
            "right from the row goes to the first cell",
            Editable::Yes,
            selected(0, 2),
            TableIn::StepCell(CellStep::Right),
            at_cell(2, Some(0), TypingState::Idle, none),
            vec![],
        ),
        (
            "right goes on to the next cell",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Idle, none),
            TableIn::StepCell(CellStep::Right),
            at_cell(2, Some(1), TypingState::Idle, none),
            vec![],
        ),
        (
            "right stops at the last cell",
            Editable::Yes,
            at_cell(2, Some(2), TypingState::Idle, none),
            TableIn::StepCell(CellStep::Right),
            at_cell(2, Some(2), TypingState::Idle, none),
            vec![],
        ),
        (
            "left stops at the first cell",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Idle, none),
            TableIn::StepCell(CellStep::Left),
            at_cell(2, Some(0), TypingState::Idle, none),
            vec![],
        ),
        (
            "the arrows keep the cell cursor on a read-only file where it was",
            Editable::No,
            selected(0, 2),
            TableIn::StepCell(CellStep::Right),
            selected(0, 2),
            vec![],
        ),
        (
            "a click on a cell picks its row and puts the cell cursor on it",
            Editable::Yes,
            selected(0, 5),
            TableIn::FocusCell(RowNo(2), ColumnNo(2)),
            at_cell(2, Some(2), TypingState::Idle, none),
            vec![],
        ),
        (
            "return takes the text and goes down a row",
            Editable::Yes,
            at_cell(2, Some(1), TypingState::Value(TypingSeed::Existing), none),
            commit(typed("hi"), AfterTyping::Down),
            at_cell(3, Some(1), TypingState::Idle, none),
            sets(2, 1, typed("hi")),
        ),
        (
            "return on the last row stays on it",
            Editable::Yes,
            at_cell(9, Some(1), TypingState::Value(TypingSeed::Existing), none),
            commit(typed("hi"), AfterTyping::Down),
            at_cell(9, Some(1), TypingState::Idle, none),
            sets(9, 1, typed("hi")),
        ),
        (
            "tab takes the text and goes right a cell",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Value(TypingSeed::Existing), none),
            commit(typed("hi"), AfterTyping::Right),
            at_cell(2, Some(1), TypingState::Idle, none),
            sets(2, 0, typed("hi")),
        ),
        (
            "the caret leaving takes the text and stays",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Value(TypingSeed::Existing), none),
            commit(typed("hi"), AfterTyping::Stay),
            at_cell(2, Some(0), TypingState::Idle, none),
            sets(2, 0, typed("hi")),
        ),
        (
            "escape lets the text go",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Value(TypingSeed::Existing), none),
            TableIn::Typing(TypingIn::Cancel),
            at_cell(2, Some(0), TypingState::Idle, none),
            vec![],
        ),
        (
            "text taken when nothing was being typed is no edit",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Idle, none),
            commit(typed("hi"), AfterTyping::Down),
            at_cell(2, Some(0), TypingState::Idle, none),
            vec![],
        ),
        (
            "a row is added below the cursor in the file's order",
            Editable::Yes,
            TableStage::Selected {
                sheet: SheetNo(0),
                rows: RowSelection::single(3),
                sort: up(1),
                find: None,
                scale: ContentScale::ACTUAL,
                cells: Cells::NONE,
            },
            TableIn::AddRow,
            selected(0, 3),
            vec![TableOut::AddRow {
                below: RowNo(3),
                sort: up(1),
            }],
        ),
        (
            "rows taken away leave none picked and the sort put away",
            Editable::Yes,
            several(0, (2, 4), 2..5, up(1)),
            TableIn::DeleteRows,
            browsing(0),
            vec![TableOut::DeleteRows {
                rows: RowSelection::of_rows(Some(2), Some(4), 2..5),
                sort: up(1),
            }],
        ),
        (
            "a read-only file takes no row away",
            Editable::No,
            selected(0, 3),
            TableIn::DeleteRows,
            selected(0, 3),
            vec![],
        ),
        (
            "undo is asked for only when there is something to take back",
            Editable::Yes,
            at_cell(2, Some(0), TypingState::Idle, unsaved(1)),
            TableIn::Session(EditSessionIn::Undo),
            at_cell(2, Some(0), TypingState::Idle, unsaved(1)),
            vec![TableOut::Session(EditSessionOut::Rewind(Rewind::Undo))],
        ),
        (
            "undo with nothing to take back asks for nothing",
            Editable::Yes,
            selected(0, 2),
            TableIn::Session(EditSessionIn::Undo),
            selected(0, 2),
            vec![],
        ),
        (
            "save is asked for only while there are changes",
            Editable::Yes,
            selected(0, 2),
            TableIn::Session(EditSessionIn::Save),
            selected(0, 2),
            vec![],
        ),
        (
            "saving the changes asks the window to write them",
            Editable::Yes,
            at_cell(2, None, TypingState::Idle, unsaved(1)),
            TableIn::Session(EditSessionIn::Save),
            at_cell(2, None, TypingState::Idle, unsaved(1)),
            vec![TableOut::Session(EditSessionOut::Save)],
        ),
        (
            "letting the changes go says so and marks them saved",
            Editable::Yes,
            at_cell(2, None, TypingState::Idle, unsaved(1)),
            TableIn::Session(EditSessionIn::Discard),
            at_cell(
                2,
                None,
                TypingState::Idle,
                EditLog {
                    changes: Changes::Saved,
                    undo: EditDepth(1),
                    ..EditLog::NONE
                },
            ),
            vec![TableOut::Session(EditSessionOut::Discard)],
        ),
        (
            "what the window says of the edits is taken as it is",
            Editable::Yes,
            selected(0, 2),
            TableIn::Session(EditSessionIn::Logged(unsaved(2))),
            at_cell(2, None, TypingState::Idle, unsaved(2)),
            vec![],
        ),
        (
            "a header press is ignored once rows came or went",
            Editable::Yes,
            at_cell(
                2,
                None,
                TypingState::Idle,
                EditLog {
                    rows: RowOrder::Changed,
                    ..EditLog::NONE
                },
            ),
            TableIn::PressHeader(1),
            at_cell(
                2,
                None,
                TypingState::Idle,
                EditLog {
                    rows: RowOrder::Changed,
                    ..EditLog::NONE
                },
            ),
            vec![],
        ),
        (
            "escape with the cursor put away leaves the cells too",
            Editable::Yes,
            at_cell(2, Some(1), TypingState::Idle, unsaved(1)),
            TableIn::Deselect,
            TableStage::Browsing {
                sheet: SheetNo(0),
                sort: None,
                find: None,
                scale: ContentScale::ACTUAL,
                cells: Cells {
                    focus: CellFocus::Row,
                    edits: DataEdits {
                        log: unsaved(1),
                        typing: TypingState::Idle,
                    },
                },
            },
            vec![],
        ),
    ];
    for (name, editable, before, input, after, want) in cases {
        let params = TableParams {
            editable,
            ..EDITABLE
        };
        let (next, outs) = before.step(input, Stamp(0), &params, &());
        assert_eq!(next, after, "{name}: state");
        assert_eq!(outs, want, "{name}: outputs");
    }
}
