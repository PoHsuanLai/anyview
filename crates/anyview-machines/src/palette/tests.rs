use super::*;
use crate::command::{Command, StageCommand};
use crate::keys::{Act, Press};
use crate::stage::HitIndex;
use crate::typed::TypedText;
use anyview_core::FileAction;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};

const EXPORT: Command = Command::File(FileAction::Export);
const ROTATE: Command = Command::File(FileAction::RotateRight);
const FIND: Command = Command::Stage(StageCommand::Find);
const ROWS: &[Command] = &[EXPORT, ROTATE, FIND];
const HIT_ROWS: &[Command] = &[
    Command::FindHit(HitIndex(0)),
    Command::FindHit(HitIndex(1)),
    Command::ShowAllHits,
    EXPORT,
];

const fn open(query: &'static str, row: usize) -> Palette {
    Palette::open(TypedText::from_static(query), PaletteIndex(row))
}

const COMMANDS: PaletteScope = PaletteScope::Commands;
const BRIEF: PaletteScope = PaletteScope::Find(HitList::Brief);
const WHOLE: PaletteScope = PaletteScope::Find(HitList::Whole);

/// Name, ranked rows, state and scope before, input, state and scope after, outputs. The generic
/// walk (moves, Enter, a click, Esc, the clamps) is quire's to test; these are the scope's.
type Case = (
    &'static str,
    &'static [Command],
    (Palette, PaletteScope),
    PaletteIn,
    (Palette, PaletteScope),
    &'static [PaletteOut],
);

const CASES: &[Case] = &[
    (
        "the find row makes what is typed a find and stays open",
        ROWS,
        (open("fox", 2), COMMANDS),
        PaletteIn::Pick(PaletteIndex(2)),
        (open("fox", 0), BRIEF),
        &[],
    ),
    (
        "command f opens a closed palette as a find on the last text",
        ROWS,
        (Palette::Closed, COMMANDS),
        PaletteIn::OpenFind(TypedText::from_static("fox")),
        (open("fox", 0), BRIEF),
        &[PaletteOut::Opened],
    ),
    (
        "command f in an open palette keeps the text and lists the hits",
        ROWS,
        (open("fox", 1), COMMANDS),
        PaletteIn::ToFind,
        (open("fox", 0), BRIEF),
        &[],
    ),
    (
        "command f in a find changes nothing",
        HIT_ROWS,
        (open("fox", 1), WHOLE),
        PaletteIn::ToFind,
        (open("fox", 1), WHOLE),
        &[],
    ),
    (
        "command f on a closed palette is nothing",
        ROWS,
        (Palette::Closed, COMMANDS),
        PaletteIn::ToFind,
        (Palette::Closed, COMMANDS),
        &[],
    ),
    (
        "typing in a find lists the first hits again",
        HIT_ROWS,
        (open("fo", 2), WHOLE),
        PaletteIn::Typed(TypedText::from_static("fox")),
        (open("fox", 0), BRIEF),
        &[],
    ),
    (
        "typing in the commands stays in the commands",
        ROWS,
        (open("ex", 2), COMMANDS),
        PaletteIn::Typed(TypedText::from_static("exp")),
        (open("exp", 0), COMMANDS),
        &[],
    ),
    (
        "show all lists every hit and stays open",
        HIT_ROWS,
        (open("fox", 2), BRIEF),
        PaletteIn::Enter,
        (open("fox", 2), WHOLE),
        &[],
    ),
    (
        "a hit runs and closes the palette, and the scope with it",
        HIT_ROWS,
        (open("fox", 1), BRIEF),
        PaletteIn::Enter,
        (Palette::Closed, COMMANDS),
        &[
            PaletteOut::Run(Command::FindHit(HitIndex(1))),
            PaletteOut::Closed,
        ],
    ),
    (
        "a command runs and closes in the commands",
        ROWS,
        (open("", 1), COMMANDS),
        PaletteIn::Enter,
        (Palette::Closed, COMMANDS),
        &[PaletteOut::Run(ROTATE), PaletteOut::Closed],
    ),
    (
        "escape closes a find, and the scope with it",
        HIT_ROWS,
        (open("fox", 1), WHOLE),
        PaletteIn::Close,
        (Palette::Closed, COMMANDS),
        &[PaletteOut::Closed],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, rows, (palette, scope), input, (state, after), outs) in CASES {
        let params = PaletteParams {
            rows: rows.to_vec(),
        };
        let (next, next_scope, out) =
            step_with_scope(palette.clone(), *scope, input.clone(), Stamp(0), &params);
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(next_scope, *after, "{name}: scope");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
    }
}

#[test]
fn keys_mean_moves_enter_and_escape() {
    // name, keys, input
    let key = |key| Press::Key(Shortcut(vec![key]));
    // name, the press, input
    let presses = [
        (
            "up",
            key(ShortcutKey::Up),
            Some(PaletteIn::Move(PaletteMove::Up)),
        ),
        (
            "down",
            key(ShortcutKey::Down),
            Some(PaletteIn::Move(PaletteMove::Down)),
        ),
        ("enter", key(ShortcutKey::Enter), Some(PaletteIn::Enter)),
        ("escape", key(ShortcutKey::Escape), Some(PaletteIn::Close)),
        (
            "the palette's action closes",
            Press::Act(Act::Palette),
            Some(PaletteIn::Close),
        ),
        (
            "a letter is typed, not a key",
            key(ShortcutKey::Char('a')),
            None,
        ),
    ];
    for (name, press, want) in presses {
        assert_eq!(PaletteIn::from_press(&press), want, "{name}");
    }
}
