//! What the palette can run: a file action, or a command for the stage that is showing.

use crate::keys::{Act, Press};
use anyview_core::{FileAction, Helper};
use ds_core::vocab::{Shortcut, ShortcutKey};
use ds_core::word::Word;

/// A command for the open picture's edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureCommand {
    /// Choose a new size for the picture.
    AdjustSize,
    /// Write the changes into the file (⌘S).
    Save,
}

impl PictureCommand {
    /// The words the palette lists the command by.
    pub fn label(self) -> &'static str {
        match self {
            PictureCommand::AdjustSize => "Adjust Size\u{2026}",
            PictureCommand::Save => "Save",
        }
    }
}

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
    /// Edit the text in place.
    Edit,
    /// Finish editing the text and go back to reading it.
    Done,
    /// Save the edited text to its file.
    Save,
}

/// The commands a chord stands for. The first row of a command is the one the palette shows.
const CHORDS: &[(Act, StageCommand)] = &[
    (Act::ZoomIn, StageCommand::ZoomIn),
    (Act::ZoomOut, StageCommand::ZoomOut),
    (Act::ZoomToFit, StageCommand::ZoomToFit),
    (Act::ZoomToActual, StageCommand::ZoomToActual),
    (Act::Find, StageCommand::Find),
    (Act::FindNext, StageCommand::FindNext),
    (Act::FindPrevious, StageCommand::FindPrevious),
    (Act::DeletePage, StageCommand::DeletePage),
    (Act::MovePageEarlier, StageCommand::MovePageEarlier),
    (Act::MovePageLater, StageCommand::MovePageLater),
    (Act::NextSheet, StageCommand::NextSheet),
    (Act::PreviousSheet, StageCommand::PreviousSheet),
    (Act::Done, StageCommand::Done),
    (Act::Save, StageCommand::Save),
];

/// The commands a key with no command modifier stands for. The first row of a command is the one
/// the palette shows when it has no chord.
const KEYS: &[(&[ShortcutKey], StageCommand)] = &[
    (&[ShortcutKey::Char('+')], StageCommand::ZoomIn),
    (&[ShortcutKey::Char('=')], StageCommand::ZoomIn),
    (&[ShortcutKey::Char('-')], StageCommand::ZoomOut),
    (&[ShortcutKey::Char('0')], StageCommand::ZoomToActual),
    (&[ShortcutKey::Char('9')], StageCommand::ZoomToFit),
    (&[ShortcutKey::Char('2')], StageCommand::ZoomToWidth),
    (&[ShortcutKey::Char('v')], StageCommand::ToggleSource),
    (&[ShortcutKey::Char('w')], StageCommand::ToggleWrap),
    (&[ShortcutKey::Space], StageCommand::TogglePlayback),
    (
        &[ShortcutKey::Shift, ShortcutKey::Left],
        StageCommand::SeekBack,
    ),
    (
        &[ShortcutKey::Shift, ShortcutKey::Right],
        StageCommand::SeekForward,
    ),
    (&[ShortcutKey::PageDown], StageCommand::NextPage),
    (&[ShortcutKey::PageUp], StageCommand::PreviousPage),
    (&[ShortcutKey::Up], StageCommand::LineUp),
    (&[ShortcutKey::Down], StageCommand::LineDown),
    (&[ShortcutKey::Home], StageCommand::ScrollToStart),
    (&[ShortcutKey::End], StageCommand::ScrollToEnd),
    (&[ShortcutKey::Char('[')], StageCommand::SlowDown),
    (&[ShortcutKey::Char(']')], StageCommand::SpeedUp),
    (&[ShortcutKey::Backspace], StageCommand::NormalSpeed),
    (&[ShortcutKey::Char('n')], StageCommand::NextChapter),
    (&[ShortcutKey::Char('p')], StageCommand::PreviousChapter),
    (&[ShortcutKey::Char('a')], StageCommand::NextAudioTrack),
    (&[ShortcutKey::Char('s')], StageCommand::NextSubtitles),
    (&[ShortcutKey::Char('.')], StageCommand::StepFrameForward),
    (&[ShortcutKey::Char(',')], StageCommand::StepFrameBack),
    (&[ShortcutKey::Char('i')], StageCommand::MarkTrimStart),
    (&[ShortcutKey::Char('o')], StageCommand::MarkTrimEnd),
    (&[ShortcutKey::Char('c')], StageCommand::CollapseAll),
    (&[ShortcutKey::Enter], StageCommand::Edit),
];

impl StageCommand {
    /// The command a press stands for, before the stage has said whether it has it: the command of
    /// the chord's action, or of the plain key. Modifiers come first in a plain key, in the
    /// order `Shortcut::keys` normalises to.
    pub fn from_press(press: &Press) -> Option<StageCommand> {
        match press {
            Press::Act(act) => CHORDS
                .iter()
                .find(|(known, _)| known == act)
                .map(|(_, command)| *command),
            Press::Key(shortcut) => {
                let keys = shortcut.keys();
                KEYS.iter()
                    .find(|(known, _)| *known == keys.as_slice())
                    .map(|(_, command)| *command)
            }
        }
    }

    /// The keys the palette shows beside the command: its action's chord as the keymap binds it,
    /// else the first plain key that turns back into it.
    pub fn shortcut(self) -> Option<Shortcut> {
        CHORDS
            .iter()
            .filter(|(_, command)| *command == self)
            .find_map(|(act, _)| act.shortcut())
            .or_else(|| {
                KEYS.iter()
                    .find(|(_, command)| *command == self)
                    .map(|(keys, _)| Shortcut(keys.to_vec()))
            })
    }
}

impl Command {
    /// The keys that do the command, which the palette draws beside it and a capsule button's
    /// tooltip after its name; `None` for a command with no key.
    #[must_use]
    pub fn shortcut(&self) -> Option<Shortcut> {
        match self {
            Command::File(action) => anyview_core::shortcut(*action),
            Command::Stage(command) => command.shortcut(),
            Command::OpenFile => Act::OpenFile.shortcut(),
            Command::UseTool(tool) => match tool {
                crate::Tool::Crop => Some(Shortcut(vec![ShortcutKey::Char('c')])),
                crate::Tool::Select | crate::Tool::Pan => {
                    Some(Shortcut(vec![ShortcutKey::Char('h')]))
                }
            },
            Command::Picture(command) => match command {
                PictureCommand::Save => Act::Save.shortcut(),
                PictureCommand::AdjustSize => None,
            },
            Command::ShowView(_) => Some(Shortcut(vec![ShortcutKey::Char('v')])),
            Command::FindHit(_) | Command::ShowAllHits | Command::Install(_) => None,
        }
    }

    /// The words the palette lists the command by.
    pub fn label(&self) -> String {
        match self {
            Command::File(action) => action.label().to_string(),
            Command::Stage(command) => command.label().to_string(),
            Command::OpenFile => "Open\u{2026}".to_owned(),
            Command::UseTool(tool) => match tool {
                crate::Tool::Crop => "Crop".to_owned(),
                crate::Tool::Select | crate::Tool::Pan => format!("Use {}", tool.label()),
            },
            Command::Picture(command) => command.label().to_owned(),
            Command::FindHit(hit) => format!("Match {}", hit.0 + 1),
            Command::ShowAllHits => "Show All Matches".to_owned(),
            Command::ShowView(view) => match view {
                crate::TextView::Rendered => "Show Preview".to_owned(),
                crate::TextView::Source => "Show Source".to_owned(),
            },
            Command::Install(_) => "Install\u{2026}".to_owned(),
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
    /// Choose another file to open, as ⌘O does: offered only where there is a file chooser.
    OpenFile,
    /// Choose the pointer tool on a picture, as Preview's tool control does. The palette lists the
    /// tool the person is not using, named for what the row switches to ("Use Select").
    UseTool(crate::Tool),
    /// A command for the picture being edited: Adjust Size or Save.
    Picture(PictureCommand),
    /// Show a text file as its page or as its source, as the titlebar's Preview | Source does. The
    /// palette lists the view the person is not looking at ("Show Source").
    ShowView(crate::TextView),
    /// Go to this hit of the find that is up. Only the palette lists it, in "In This File".
    FindHit(crate::HitIndex),
    /// List every hit of the find, not the first few. Only the palette lists it.
    ShowAllHits,
    /// Offer to install the tool the open file needs: the Install… of a `Needs` row. The palette
    /// never lists it; a row of the stage's own sends it.
    Install(Helper),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_the_press_that_means_it_and_a_shortcut_to_show() {
        for command in StageCommand::ALL {
            let chord = CHORDS
                .iter()
                .find(|(_, known)| known == command)
                .map(|(act, _)| Press::Act(*act));
            let key = KEYS
                .iter()
                .find(|(_, known)| known == command)
                .map(|(keys, _)| Press::Key(Shortcut(keys.to_vec())));
            let press = chord
                .or(key)
                .unwrap_or_else(|| panic!("{command:?} has no press"));
            assert_eq!(StageCommand::from_press(&press), Some(*command));
            assert!(command.shortcut().is_some(), "{command:?} shows nothing");
        }
    }
}
