use super::*;
use crate::command::{Command, StageCommand};
use crate::panel::{PanelTab, PanelTabs};
use anyview_core::FileAction;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const fn run(action: FileAction) -> Command {
    Command::File(action)
}

const fn row(action: FileAction, title: &'static str) -> ContextEntry {
    ContextEntry::Item {
        pick: ContextPick::Run(run(action)),
        title,
    }
}

const INFO_ROW: ContextEntry = ContextEntry::Item {
    pick: ContextPick::GetInfo,
    title: "Get Info",
};
const RULE: ContextEntry = ContextEntry::Separator;
const INFO: PanelTabs = PanelTabs::of(&[PanelTab::Info]);

/// What a still picture's palette lists, in the palette's order.
const PICTURE: &[Command] = &[
    run(FileAction::RevealInFolder),
    run(FileAction::CopyPath),
    run(FileAction::Share),
    run(FileAction::Rename),
    run(FileAction::Duplicate),
    run(FileAction::MoveToTrash),
    run(FileAction::Print),
    run(FileAction::Export),
    run(FileAction::SaveCopy),
    run(FileAction::RotateLeft),
    run(FileAction::RotateRight),
    run(FileAction::FlipHorizontal),
    Command::Stage(StageCommand::ZoomIn),
];

#[test]
fn the_rows_follow_the_palettes_commands_in_the_context_order() {
    // name, the palette's commands, the panel's tabs, the menu
    let cases: Vec<(&str, Vec<Command>, PanelTabs, Vec<ContextEntry>)> = vec![
        (
            "a picture",
            PICTURE.to_vec(),
            INFO,
            vec![
                row(FileAction::RotateLeft, "Rotate Left"),
                row(FileAction::RotateRight, "Rotate Right"),
                RULE,
                row(FileAction::CopyPath, "Copy Path"),
                RULE,
                row(FileAction::RevealInFolder, "Show in Folder"),
                INFO_ROW,
                RULE,
                row(FileAction::Export, "Export\u{2026}"),
                row(FileAction::Share, "Share\u{2026}"),
                RULE,
                row(FileAction::Rename, "Rename\u{2026}"),
                row(FileAction::Duplicate, "Duplicate"),
                row(FileAction::MoveToTrash, "Move to Trash"),
            ],
        ),
        (
            "a picture whose edit is withheld has no rotate and no rule where it was",
            vec![
                run(FileAction::RevealInFolder),
                run(FileAction::MoveToTrash),
            ],
            INFO,
            vec![
                row(FileAction::RevealInFolder, "Show in Folder"),
                INFO_ROW,
                RULE,
                row(FileAction::MoveToTrash, "Move to Trash"),
            ],
        ),
        (
            "a recording starts with its playback",
            vec![
                Command::Stage(StageCommand::TogglePlayback),
                Command::Stage(StageCommand::StepFrameForward),
                Command::Stage(StageCommand::StepFrameBack),
                Command::Stage(StageCommand::SeekBack),
                run(FileAction::RevealInFolder),
            ],
            PanelTabs::NONE,
            vec![
                ContextEntry::Item {
                    pick: ContextPick::Run(Command::Stage(StageCommand::TogglePlayback)),
                    title: "Play/Pause",
                },
                ContextEntry::Item {
                    pick: ContextPick::Run(Command::Stage(StageCommand::StepFrameBack)),
                    title: "Previous Frame",
                },
                ContextEntry::Item {
                    pick: ContextPick::Run(Command::Stage(StageCommand::StepFrameForward)),
                    title: "Next Frame",
                },
                RULE,
                row(FileAction::RevealInFolder, "Show in Folder"),
            ],
        ),
        (
            "a file copy is a row only when the palette lists one",
            vec![run(FileAction::CopyFile)],
            PanelTabs::NONE,
            vec![row(FileAction::CopyFile, "Copy")],
        ),
        ("nothing is listed", vec![], PanelTabs::NONE, vec![]),
        ("only Get Info", vec![], INFO, vec![INFO_ROW]),
    ];
    for (name, commands, tabs, want) in cases {
        assert_eq!(entries(&commands, tabs), want, "{name}");
    }
}

fn params(entries: Vec<ContextEntry>) -> ContextParams {
    ContextParams {
        entries,
        centre: Spot { x: 450, y: 300 },
    }
}

const HERE: Spot = Spot { x: 12, y: 34 };
const THERE: Spot = Spot { x: 99, y: 7 };
const ROTATE: ContextPick = ContextPick::Run(run(FileAction::RotateLeft));

#[test]
fn the_menu_opens_where_it_is_told_and_runs_a_pick_before_it_closes() {
    // name, before, input, after, outputs
    let cases: Vec<(&str, ContextMenu, ContextIn, ContextMenu, Vec<ContextOut>)> = vec![
        (
            "a secondary click opens at the pointer",
            ContextMenu::Closed,
            ContextIn::Open(HERE),
            ContextMenu::Open { at: HERE },
            vec![],
        ),
        (
            "the Menu key opens at the middle of the content",
            ContextMenu::Closed,
            ContextIn::OpenAtCentre,
            ContextMenu::Open {
                at: Spot { x: 450, y: 300 },
            },
            vec![],
        ),
        (
            "another click moves the menu",
            ContextMenu::Open { at: HERE },
            ContextIn::Open(THERE),
            ContextMenu::Open { at: THERE },
            vec![],
        ),
        (
            "a pick runs and the menu stays for its fade",
            ContextMenu::Open { at: HERE },
            ContextIn::Pick(ROTATE),
            ContextMenu::Open { at: HERE },
            vec![ContextOut::Run(ROTATE)],
        ),
        (
            "a row the menu does not list runs nothing",
            ContextMenu::Open { at: HERE },
            ContextIn::Pick(ContextPick::Run(run(FileAction::Print))),
            ContextMenu::Open { at: HERE },
            vec![],
        ),
        (
            "close closes",
            ContextMenu::Open { at: HERE },
            ContextIn::Close,
            ContextMenu::Closed,
            vec![],
        ),
        (
            "a closed menu ignores a pick and a close",
            ContextMenu::Closed,
            ContextIn::Pick(ROTATE),
            ContextMenu::Closed,
            vec![],
        ),
    ];
    for (name, before, input, after, outs) in cases {
        let rows = vec![ContextEntry::Item {
            pick: ROTATE,
            title: "Rotate Left",
        }];
        let (next, out) = before.step(input, Stamp(0), &params(rows), &());
        assert_eq!(next, after, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: a menu keeps no timer");
    }
}

#[test]
fn a_file_with_no_rows_opens_no_menu() {
    for input in [ContextIn::Open(HERE), ContextIn::OpenAtCentre] {
        let (next, outs) = ContextMenu::Closed.step(input, Stamp(0), &params(vec![]), &());
        assert_eq!((next, outs), (ContextMenu::Closed, vec![]), "{input:?}");
    }
}
