use super::*;
use crate::stage::find::{FindHits, FindOut, HitCount, HitIndex, StageFind};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::zoom::{ContentScale, ZoomDir};
use crate::typed::TypedText;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const fn browsing(sheet: u32) -> TableStage {
    browsing_by(sheet, None)
}
const fn selected(sheet: u32, row: u32) -> TableStage {
    selected_by(sheet, row, None)
}
const fn browsing_by(sheet: u32, sort: Option<ColumnSort>) -> TableStage {
    TableStage::Browsing {
        sheet: SheetNo(sheet),
        sort,
        find: None,
        scale: ContentScale::ACTUAL,
    }
}
const fn selected_by(sheet: u32, row: u32, sort: Option<ColumnSort>) -> TableStage {
    TableStage::Selected {
        sheet: SheetNo(sheet),
        row: RowNo(row),
        sort,
        find: None,
        scale: ContentScale::ACTUAL,
    }
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

const CASES: &[Case] = &[
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
        "a sort drops the cursor, whose row is elsewhere now",
        1,
        selected(0, 4),
        TableIn::PressHeader(0),
        browsing_by(0, up(0)),
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
];

#[test]
fn the_table_stage_steps_as_the_table_says() {
    for (name, sheets, before, input, after) in CASES {
        let params = TableParams {
            sheets: SheetTotal(*sheets),
            columns: 3,
            rows: 10,
            page: 4,
        };
        let (next, outs) = before.clone().step(input.clone(), Stamp(0), &params, &());
        assert_eq!(next, *after, "{name}");
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
}

const CAT: TypedText = TypedText::from_static("cat");

fn finding(sheet: u32, hits: FindHits) -> TableStage {
    TableStage::Browsing {
        sheet: SheetNo(sheet),
        sort: None,
        find: Some(StageFind { query: CAT, hits }),
        scale: ContentScale::ACTUAL,
    }
}

fn found(count: u32, at: u32) -> FindHits {
    FindHits::answered(HitCount(count), HitIndex(at))
}

fn zoomed(stage: TableStage, scale: ContentScale) -> TableStage {
    match stage {
        TableStage::Browsing {
            sheet, sort, find, ..
        } => TableStage::Browsing {
            sheet,
            sort,
            find,
            scale,
        },
        TableStage::Selected {
            sheet,
            row,
            sort,
            find,
            ..
        } => TableStage::Selected {
            sheet,
            row,
            sort,
            find,
            scale,
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
                row: RowNo(4),
                sort: None,
                find: Some(StageFind {
                    query: CAT,
                    hits: found(3, 1),
                }),
                scale: ContentScale::ACTUAL,
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
