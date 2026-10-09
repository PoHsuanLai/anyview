//! Which rows the context menu has, and in what order. The rows are chosen from the commands the
//! palette lists for the open file, so the menu, the palette and the shortcuts offer the same
//! things; this only says which of them a Mac's context menu shows, how it names them and where
//! the rules go.

use super::model::{ContextEntry, ContextPick};
use crate::command::{Command, StageCommand};
use crate::panel::{PanelTab, PanelTabs};
use anyview_core::FileAction;

const fn file(action: FileAction, title: &'static str) -> (ContextPick, &'static str) {
    (ContextPick::Run(Command::File(action)), title)
}

const fn stage(command: StageCommand, title: &'static str) -> (ContextPick, &'static str) {
    (ContextPick::Run(Command::Stage(command)), title)
}

/// The rows in the order Preview shows them, a group at a time; a rule goes between the groups
/// that have a row. A recording's playback comes first, since it is what a click on it means.
const GROUPS: &[&[(ContextPick, &str)]] = &[
    &[
        stage(StageCommand::TogglePlayback, "Play/Pause"),
        stage(StageCommand::StepFrameBack, "Previous Frame"),
        stage(StageCommand::StepFrameForward, "Next Frame"),
    ],
    &[
        file(FileAction::RotateLeft, "Rotate Left"),
        file(FileAction::RotateRight, "Rotate Right"),
    ],
    &[
        file(FileAction::CopyFile, "Copy"),
        file(FileAction::CopyPath, "Copy Path"),
    ],
    &[
        (ContextPick::Run(Command::OpenFile), "Open\u{2026}"),
        file(FileAction::RevealInFolder, "Show in Folder"),
        (ContextPick::GetInfo, "Get Info"),
    ],
    &[
        file(FileAction::Export, "Export\u{2026}"),
        file(FileAction::Share, "Share\u{2026}"),
    ],
    &[
        file(FileAction::Rename, "Rename\u{2026}"),
        file(FileAction::Duplicate, "Duplicate"),
        file(FileAction::MoveToTrash, "Move to Trash"),
    ],
];

/// The menu for a file whose palette lists `commands` and whose side panel has `tabs`.
pub fn entries(commands: &[Command], tabs: PanelTabs) -> Vec<ContextEntry> {
    let listed = |pick: ContextPick| match pick {
        ContextPick::Run(command) => commands.contains(&command),
        ContextPick::GetInfo => tabs.contains(PanelTab::Info),
    };
    GROUPS
        .iter()
        .map(|group| {
            group
                .iter()
                .filter(|(pick, _)| listed(*pick))
                .map(|(pick, title)| ContextEntry::Item { pick: *pick, title })
                .collect::<Vec<_>>()
        })
        .filter(|rows| !rows.is_empty())
        .enumerate()
        .flat_map(|(at, rows)| {
            let rule = (at > 0).then_some(ContextEntry::Separator);
            rule.into_iter().chain(rows)
        })
        .collect()
}
