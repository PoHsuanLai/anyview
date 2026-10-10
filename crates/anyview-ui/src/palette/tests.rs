use super::*;
use crate::command::{Command, StageCommand};
use crate::stage::HitIndex;
use crate::typed::TypedText;
use anyview_core::FileAction;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::ShortcutKey;

const EXPORT: Command = Command::File(FileAction::Export);
const ROTATE: Command = Command::File(FileAction::RotateRight);
const FIND: Command = Command::Stage(StageCommand::Find);
const ROWS: &[Command] = &[EXPORT, ROTATE, FIND];

const fn open(query: &'static str, row: usize) -> Palette {
    Palette::Open {
        query: TypedText::from_static(query),
        selection: PaletteIndex(row),
        scope: PaletteScope::Commands,
    }
}

const fn finding(query: &'static str, row: usize, list: HitList) -> Palette {
    Palette::Open {
        query: TypedText::from_static(query),
        selection: PaletteIndex(row),
        scope: PaletteScope::Find(list),
    }
}

const HIT_ROWS: &[Command] = &[
    Command::FindHit(HitIndex(0)),
    Command::FindHit(HitIndex(1)),
    Command::ShowAllHits,
    EXPORT,
];

/// Name, ranked rows, state before, input, state after, outputs.
type Case = (
    &'static str,
    &'static [Command],
    Palette,
    PaletteIn,
    Palette,
    &'static [PaletteOut],
);

const CASES: &[Case] = &[
    (
        "open shows an empty query on the first row",
        ROWS,
        Palette::Closed,
        PaletteIn::Open,
        open("", 0),
        &[PaletteOut::Opened],
    ),
    (
        "typing replaces the query and returns to the first row",
        ROWS,
        open("ex", 2),
        PaletteIn::Typed(TypedText::from_static("exp")),
        open("exp", 0),
        &[],
    ),
    (
        "down moves one row",
        ROWS,
        open("", 0),
        PaletteIn::Move(PaletteMove::Down),
        open("", 1),
        &[],
    ),
    (
        "down stops at the last row",
        ROWS,
        open("", 2),
        PaletteIn::Move(PaletteMove::Down),
        open("", 2),
        &[],
    ),
    (
        "up stops at the first row",
        ROWS,
        open("", 0),
        PaletteIn::Move(PaletteMove::Up),
        open("", 0),
        &[],
    ),
    (
        "last jumps to the end",
        ROWS,
        open("", 0),
        PaletteIn::Move(PaletteMove::Last),
        open("", 2),
        &[],
    ),
    (
        "first jumps to the start",
        ROWS,
        open("", 2),
        PaletteIn::Move(PaletteMove::First),
        open("", 0),
        &[],
    ),
    (
        "moving over no rows stays on row zero",
        &[],
        open("zzz", 0),
        PaletteIn::Move(PaletteMove::Down),
        open("zzz", 0),
        &[],
    ),
    (
        "a shorter list pulls a stale selection back in range",
        &[EXPORT],
        open("e", 2),
        PaletteIn::Move(PaletteMove::Up),
        open("e", 0),
        &[],
    ),
    (
        "enter runs the highlighted row and closes",
        ROWS,
        open("", 1),
        PaletteIn::Enter,
        Palette::Closed,
        &[PaletteOut::Run(ROTATE), PaletteOut::Closed],
    ),
    (
        "enter with no rows stays open",
        &[],
        open("zzz", 0),
        PaletteIn::Enter,
        open("zzz", 0),
        &[],
    ),
    (
        "a click runs the row it landed on",
        ROWS,
        open("", 0),
        PaletteIn::Pick(PaletteIndex(1)),
        Palette::Closed,
        &[PaletteOut::Run(ROTATE), PaletteOut::Closed],
    ),
    (
        "the find row makes what is typed a find and stays open",
        ROWS,
        open("fox", 2),
        PaletteIn::Pick(PaletteIndex(2)),
        finding("fox", 0, HitList::Brief),
        &[],
    ),
    (
        "command f opens a closed palette as a find on the last text",
        ROWS,
        Palette::Closed,
        PaletteIn::OpenFind(TypedText::from_static("fox")),
        finding("fox", 0, HitList::Brief),
        &[PaletteOut::Opened],
    ),
    (
        "command f in an open palette keeps the text and lists the hits",
        ROWS,
        open("fox", 1),
        PaletteIn::ToFind,
        finding("fox", 0, HitList::Brief),
        &[],
    ),
    (
        "typing in a find lists the first hits again",
        HIT_ROWS,
        finding("fo", 2, HitList::Whole),
        PaletteIn::Typed(TypedText::from_static("fox")),
        finding("fox", 0, HitList::Brief),
        &[],
    ),
    (
        "show all lists every hit and stays open",
        HIT_ROWS,
        finding("fox", 2, HitList::Brief),
        PaletteIn::Enter,
        finding("fox", 2, HitList::Whole),
        &[],
    ),
    (
        "a hit runs and closes the palette",
        HIT_ROWS,
        finding("fox", 1, HitList::Brief),
        PaletteIn::Enter,
        Palette::Closed,
        &[
            PaletteOut::Run(Command::FindHit(HitIndex(1))),
            PaletteOut::Closed,
        ],
    ),
    (
        "a click past the rows is nothing",
        ROWS,
        open("", 0),
        PaletteIn::Pick(PaletteIndex(9)),
        open("", 0),
        &[],
    ),
    (
        "escape closes",
        ROWS,
        open("ex", 1),
        PaletteIn::Close,
        Palette::Closed,
        &[PaletteOut::Closed],
    ),
    (
        "opening again keeps the query",
        ROWS,
        open("ex", 1),
        PaletteIn::Open,
        open("ex", 1),
        &[],
    ),
    (
        "a closed palette ignores enter",
        ROWS,
        Palette::Closed,
        PaletteIn::Enter,
        Palette::Closed,
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, rows, from, input, state, outs) in CASES {
        let params = PaletteParams {
            rows: rows.to_vec(),
        };
        let (next, out) = from.clone().step(input.clone(), Stamp(0), &params, &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: a palette keeps no timer");
    }
}

#[test]
fn keys_mean_moves_enter_and_escape() {
    // name, keys, input
    const KEYS: &[(&str, &[ShortcutKey], Option<PaletteIn>)] = &[
        (
            "up",
            &[ShortcutKey::Up],
            Some(PaletteIn::Move(PaletteMove::Up)),
        ),
        (
            "down",
            &[ShortcutKey::Down],
            Some(PaletteIn::Move(PaletteMove::Down)),
        ),
        ("enter", &[ShortcutKey::Enter], Some(PaletteIn::Enter)),
        ("escape", &[ShortcutKey::Escape], Some(PaletteIn::Close)),
        (
            "command k closes",
            &[ShortcutKey::Super, ShortcutKey::Char('k')],
            Some(PaletteIn::Close),
        ),
        (
            "a letter is typed, not a key",
            &[ShortcutKey::Char('a')],
            None,
        ),
    ];
    for (name, keys, want) in KEYS {
        assert_eq!(PaletteIn::from_key(keys), *want, "{name}");
    }
}
