//! What the palette can run: a file action, or a command for the stage that is showing.

use anyview_core::FileAction;
use ds_core::vocab::{Shortcut, ShortcutKey};
use ds_core::word::Word;

/// A command a stage understands. The stage that is showing decides what it means, or that it
/// means nothing (a PDF has no "toggle source").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StageCommand {
    /// Zoom in one step.
    ZoomIn,
    /// Zoom out one step.
    ZoomOut,
    /// Fit the content to the window.
    ZoomToFit,
    /// Fit the content's width to the window.
    ZoomToWidth,
    /// Show the content at its own size.
    ZoomToActual,
    /// Start a find.
    Find,
    /// Go to the next find hit.
    FindNext,
    /// Go to the previous find hit.
    FindPrevious,
    /// Switch a text document between rendered and source.
    ToggleSource,
    /// Switch line wrapping.
    ToggleWrap,
    /// Play or pause.
    TogglePlayback,
    /// Seek back by one step.
    SeekBack,
    /// Seek forward by one step.
    SeekForward,
    /// The next page.
    NextPage,
    /// The previous page.
    PreviousPage,
}

impl StageCommand {
    /// The command a key stands for, before the stage has said whether it has it. Modifiers come
    /// first, in the order `Shortcut::keys` normalises to.
    pub fn from_key(keys: &[ShortcutKey]) -> Option<StageCommand> {
        use ShortcutKey::{Char, Left, PageDown, PageUp, Right, Shift, Space, Super};
        match keys {
            [Char('+' | '=')] | [Shift, Char('+')] | [Super, Char('+' | '=')] => {
                Some(StageCommand::ZoomIn)
            }
            [Char('-')] | [Super, Char('-')] => Some(StageCommand::ZoomOut),
            [Char('0')] | [Super, Char('0')] => Some(StageCommand::ZoomToFit),
            [Char('9')] | [Super, Char('9')] => Some(StageCommand::ZoomToWidth),
            [Char('1')] | [Super, Char('1')] => Some(StageCommand::ZoomToActual),
            [Super, Char('f')] => Some(StageCommand::Find),
            [Super, Char('g')] => Some(StageCommand::FindNext),
            [Shift, Super, Char('g')] => Some(StageCommand::FindPrevious),
            [Char('v')] => Some(StageCommand::ToggleSource),
            [Char('w')] => Some(StageCommand::ToggleWrap),
            [Space] => Some(StageCommand::TogglePlayback),
            [Shift, Left] => Some(StageCommand::SeekBack),
            [Shift, Right] => Some(StageCommand::SeekForward),
            [PageDown] => Some(StageCommand::NextPage),
            [PageUp] => Some(StageCommand::PreviousPage),
            _ => None,
        }
    }
}

impl StageCommand {
    /// The keys the palette shows beside the command: the first key `from_key` turns back into
    /// it.
    pub fn shortcut(self) -> Shortcut {
        use ShortcutKey::{Char, Left, PageDown, PageUp, Right, Shift, Space, Super};
        Shortcut(match self {
            StageCommand::ZoomIn => vec![Char('+')],
            StageCommand::ZoomOut => vec![Char('-')],
            StageCommand::ZoomToFit => vec![Char('0')],
            StageCommand::ZoomToWidth => vec![Char('9')],
            StageCommand::ZoomToActual => vec![Char('1')],
            StageCommand::Find => vec![Super, Char('f')],
            StageCommand::FindNext => vec![Super, Char('g')],
            StageCommand::FindPrevious => vec![Shift, Super, Char('g')],
            StageCommand::ToggleSource => vec![Char('v')],
            StageCommand::ToggleWrap => vec![Char('w')],
            StageCommand::TogglePlayback => vec![Space],
            StageCommand::SeekBack => vec![Shift, Left],
            StageCommand::SeekForward => vec![Shift, Right],
            StageCommand::NextPage => vec![PageDown],
            StageCommand::PreviousPage => vec![PageUp],
        })
    }
}

impl Command {
    /// The words the palette lists the command by.
    pub fn label(&self) -> String {
        match self {
            Command::File(action) => action.label().to_string(),
            Command::Stage(command) => command.label().to_string(),
        }
    }
}

/// One row the palette can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// An action every file has some of; the one list in `anyview-core`.
    File(FileAction),
    /// A command for the stage.
    Stage(StageCommand),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_the_key_that_means_it() {
        for command in StageCommand::ALL {
            let keys = command.shortcut().keys();
            assert_eq!(
                StageCommand::from_key(&keys),
                Some(*command),
                "{command:?} is shown as {keys:?}"
            );
        }
    }
}
