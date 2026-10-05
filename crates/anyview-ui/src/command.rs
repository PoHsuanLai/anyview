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
    /// One line towards the start of a text.
    LineUp,
    /// One line towards the end of a text.
    LineDown,
    /// The start of a text.
    ScrollToStart,
    /// The end of a text.
    ScrollToEnd,
    /// Play the next speed down.
    SlowDown,
    /// Play the next speed up.
    SpeedUp,
    /// Play at the recording's own speed.
    NormalSpeed,
    /// Jump to the next chapter.
    NextChapter,
    /// Jump to the previous chapter.
    PreviousChapter,
    /// Play the next audio track.
    NextAudioTrack,
    /// Show the next subtitle track, then none.
    NextSubtitles,
    /// Show the next frame.
    StepFrameForward,
    /// Show the previous frame.
    StepFrameBack,
    /// Mark where a trim starts, at the position now.
    MarkTrimStart,
    /// Mark where a trim ends, at the position now.
    MarkTrimEnd,
    /// Remove the page on screen from a PDF.
    DeletePage,
    /// Move the page on screen one place earlier in a PDF.
    MovePageEarlier,
    /// Move the page on screen one place later in a PDF.
    MovePageLater,
    /// The next sheet of a workbook.
    NextSheet,
    /// The previous sheet of a workbook.
    PreviousSheet,
    /// Close every open node of a tree but the top level.
    CollapseAll,
}

impl StageCommand {
    /// The command a key stands for, before the stage has said whether it has it. Modifiers come
    /// first, in the order `Shortcut::keys` normalises to.
    pub fn from_key(keys: &[ShortcutKey]) -> Option<StageCommand> {
        use ShortcutKey::{
            Backspace, Char, Down, End, Home, Left, PageDown, PageUp, Right, Shift, Space, Super,
            Up,
        };
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
            [Up] => Some(StageCommand::LineUp),
            [Down] => Some(StageCommand::LineDown),
            [Home] => Some(StageCommand::ScrollToStart),
            [End] => Some(StageCommand::ScrollToEnd),
            [Char('[')] => Some(StageCommand::SlowDown),
            [Char(']')] => Some(StageCommand::SpeedUp),
            [Backspace] => Some(StageCommand::NormalSpeed),
            [Char('n')] => Some(StageCommand::NextChapter),
            [Char('p')] => Some(StageCommand::PreviousChapter),
            [Char('a')] => Some(StageCommand::NextAudioTrack),
            [Char('s')] => Some(StageCommand::NextSubtitles),
            [Char('.')] => Some(StageCommand::StepFrameForward),
            [Char(',')] => Some(StageCommand::StepFrameBack),
            [Char('i')] => Some(StageCommand::MarkTrimStart),
            [Char('o')] => Some(StageCommand::MarkTrimEnd),
            [Shift, Super, Backspace] => Some(StageCommand::DeletePage),
            [Shift, Super, Up] => Some(StageCommand::MovePageEarlier),
            [Shift, Super, Down] => Some(StageCommand::MovePageLater),
            [Super, Char(']')] => Some(StageCommand::NextSheet),
            [Super, Char('[')] => Some(StageCommand::PreviousSheet),
            [Char('c')] => Some(StageCommand::CollapseAll),
            _ => None,
        }
    }
}

impl StageCommand {
    /// The keys the palette shows beside the command: the first key `from_key` turns back into
    /// it.
    pub fn shortcut(self) -> Shortcut {
        use ShortcutKey::{
            Backspace, Char, Down, End, Home, Left, PageDown, PageUp, Right, Shift, Space, Super,
            Up,
        };
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
            StageCommand::LineUp => vec![Up],
            StageCommand::LineDown => vec![Down],
            StageCommand::ScrollToStart => vec![Home],
            StageCommand::ScrollToEnd => vec![End],
            StageCommand::SlowDown => vec![Char('[')],
            StageCommand::SpeedUp => vec![Char(']')],
            StageCommand::NormalSpeed => vec![Backspace],
            StageCommand::NextChapter => vec![Char('n')],
            StageCommand::PreviousChapter => vec![Char('p')],
            StageCommand::NextAudioTrack => vec![Char('a')],
            StageCommand::NextSubtitles => vec![Char('s')],
            StageCommand::StepFrameForward => vec![Char('.')],
            StageCommand::StepFrameBack => vec![Char(',')],
            StageCommand::MarkTrimStart => vec![Char('i')],
            StageCommand::MarkTrimEnd => vec![Char('o')],
            StageCommand::DeletePage => vec![Shift, Super, Backspace],
            StageCommand::MovePageEarlier => vec![Shift, Super, Up],
            StageCommand::MovePageLater => vec![Shift, Super, Down],
            StageCommand::NextSheet => vec![Super, Char(']')],
            StageCommand::PreviousSheet => vec![Super, Char('[')],
            StageCommand::CollapseAll => vec![Char('c')],
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
