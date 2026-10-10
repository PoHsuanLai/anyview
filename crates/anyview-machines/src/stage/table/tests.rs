use super::*;
use crate::stage::row::{RowNo, RowStep};
use anyview_core::{ColumnSort, SortDirection};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const fn browsing(sheet: u32) -> TableStage {
    TableStage::Browsing {
        sheet: SheetNo(sheet),
        sort: None,
    }
}
const fn selected(sheet: u32, row: u32) -> TableStage {
    TableStage::Selected {
        sheet: SheetNo(sheet),
        row: RowNo(row),
        sort: None,
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
const fn sorted(stage: TableStage, sort: Option<ColumnSort>) -> TableStage {
    match stage {
        TableStage::Browsing { sheet, .. } => TableStage::Browsing { sheet, sort },
        TableStage::Selected { sheet, row, .. } => TableStage::Selected { sheet, row, sort },
    }
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
        sorted(browsing(0), up(1)),
    ),
    (
        "the same header again sorts descending",
        1,
        sorted(browsing(0), up(1)),
        TableIn::PressHeader(1),
        sorted(browsing(0), down(1)),
    ),
    (
        "a third press puts the sort away",
        1,
        sorted(browsing(0), down(1)),
        TableIn::PressHeader(1),
        browsing(0),
    ),
    (
        "another header sorts that column ascending",
        1,
        sorted(browsing(0), down(1)),
        TableIn::PressHeader(2),
        sorted(browsing(0), up(2)),
    ),
    (
        "a sort drops the cursor, whose row is elsewhere now",
        1,
        selected(0, 4),
        TableIn::PressHeader(0),
        sorted(browsing(0), up(0)),
    ),
    (
        "a header the sheet has not is ignored",
        1,
        sorted(selected(0, 4), up(1)),
        TableIn::PressHeader(3),
        sorted(selected(0, 4), up(1)),
    ),
    (
        "the cursor moves inside the sort",
        1,
        sorted(selected(0, 4), down(1)),
        TableIn::Move(RowStep::Down),
        sorted(selected(0, 5), down(1)),
    ),
    (
        "putting the cursor away keeps the sort",
        1,
        sorted(selected(0, 4), down(1)),
        TableIn::Deselect,
        sorted(browsing(0), down(1)),
    ),
    (
        "another sheet has other columns, so the sort goes",
        2,
        sorted(selected(0, 4), up(1)),
        TableIn::NextSheet,
        browsing(1),
    ),
    (
        "no other sheet keeps the sort",
        2,
        sorted(browsing(1), up(1)),
        TableIn::NextSheet,
        sorted(browsing(1), up(1)),
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
        let (next, outs) = before.step(*input, Stamp(0), &params, &());
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
    assert_eq!(sorted(selected(2, 7), down(1)).sort(), down(1));
}
