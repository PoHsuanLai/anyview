use super::*;
use crate::stage::row::{RowNo, RowStep};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const fn browsing(sheet: u32) -> TableStage {
    TableStage::Browsing {
        sheet: SheetNo(sheet),
    }
}
const fn selected(sheet: u32, row: u32) -> TableStage {
    TableStage::Selected {
        sheet: SheetNo(sheet),
        row: RowNo(row),
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
];

#[test]
fn the_table_stage_steps_as_the_table_says() {
    for (name, sheets, before, input, after) in CASES {
        let params = TableParams {
            sheets: SheetTotal(*sheets),
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
}
