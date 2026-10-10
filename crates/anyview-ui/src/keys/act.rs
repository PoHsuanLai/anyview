//! The viewer's actions: what a chord means once the keymap has resolved it. A standard action
//! (Find, Close, Save, Bigger…) is the keymap's own and only named here; the rest are declared with
//! a default chord and registered once per window, so a person's own change to one wins.

use anyview_core::{FileAction, file_action_of, own_keys, shortcut};
use chordkit::{Action, AppAction, AppId, DefaultChord, StandardAction};
use ds_core::vocab::Shortcut;

/// The viewer's name as the owner of its actions.
const APP: &str = "anyview";

/// What a chord the keymap resolved asks of the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Act {
    /// Open the palette (⌘K).
    Palette,
    /// Find in the open file (the standard Find).
    Find,
    /// Go to the next hit (the standard Find Next).
    FindNext,
    /// Go to the previous hit (the standard Find Previous).
    FindPrevious,
    /// Show or hide the Info tab.
    Info,
    /// Close the window (the standard Close).
    Close,
    /// Choose another file to open (the standard Open).
    OpenFile,
    /// Write the changes (the standard Save).
    Save,
    /// Take back the last edit (the standard Undo).
    Undo,
    /// Do the edit taken back again (the standard Redo).
    Redo,
    /// Zoom in one step (the standard Bigger).
    ZoomIn,
    /// Zoom out one step (the standard Smaller).
    ZoomOut,
    /// Fit the content to the window.
    ZoomToFit,
    /// Show the content at its own size.
    ZoomToActual,
    /// Finish editing the text.
    Done,
    /// Remove the page on screen from a PDF.
    DeletePage,
    /// Move the page on screen one place earlier.
    MovePageEarlier,
    /// Move the page on screen one place later.
    MovePageLater,
    /// The next sheet of a workbook.
    NextSheet,
    /// The previous sheet of a workbook.
    PreviousSheet,
    /// An action on the file.
    File(FileAction),
}

/// The viewer's own actions that are not about the file: the action's id and its default chord.
/// `Primary` is Command or Ctrl, whichever the platform means.
const OWN: &[(Act, &str, &str)] = &[
    (Act::Palette, "anyview.palette", "Primary+K"),
    (Act::Info, "anyview.info", "Primary+Alt+I"),
    (Act::ZoomToFit, "anyview.zoom-to-fit", "Primary+0"),
    (Act::ZoomToActual, "anyview.zoom-to-actual", "Primary+Alt+0"),
    (Act::Done, "anyview.done", "Primary+Enter"),
    (
        Act::DeletePage,
        "anyview.delete-page",
        "Primary+Shift+Backspace",
    ),
    (
        Act::MovePageEarlier,
        "anyview.move-page-earlier",
        "Primary+Shift+Up",
    ),
    (
        Act::MovePageLater,
        "anyview.move-page-later",
        "Primary+Shift+Down",
    ),
    (Act::NextSheet, "anyview.next-sheet", "Primary+PageDown"),
    (
        Act::PreviousSheet,
        "anyview.previous-sheet",
        "Primary+PageUp",
    ),
];

/// The standard actions the viewer answers, and what each is here.
const STANDARD: &[(StandardAction, Act)] = &[
    (StandardAction::Find, Act::Find),
    (StandardAction::FindNext, Act::FindNext),
    (StandardAction::FindPrevious, Act::FindPrevious),
    (StandardAction::Close, Act::Close),
    (StandardAction::Open, Act::OpenFile),
    (StandardAction::Save, Act::Save),
    (StandardAction::Undo, Act::Undo),
    (StandardAction::Redo, Act::Redo),
    (StandardAction::Bigger, Act::ZoomIn),
    (StandardAction::Smaller, Act::ZoomOut),
];

impl Act {
    /// What the viewer makes of a keymap action, or `None` for one it has no use for.
    #[must_use]
    pub fn of(action: &Action) -> Option<Act> {
        match action {
            Action::Standard(standard) => STANDARD
                .iter()
                .find(|(known, _)| known == standard)
                .map(|(_, act)| *act)
                .or_else(|| file_action_of(action).map(Act::File)),
            Action::App(app) => OWN
                .iter()
                .find(|(_, id, _)| *id == app.id())
                .map(|(act, _, _)| *act)
                .or_else(|| file_action_of(action).map(Act::File)),
            _ => None,
        }
    }

    /// The keys that do it, for a menu, a tooltip or a palette row to draw: a standard action's
    /// are drawn as the keymap binds them, the viewer's own as they were declared.
    #[must_use]
    pub fn shortcut(self) -> Option<Shortcut> {
        let found = if let Act::File(action) = self {
            shortcut(action)
        } else {
            STANDARD
                .iter()
                .find(|(_, act)| *act == self)
                .map(|(standard, _)| Shortcut::standard(*standard))
                .or_else(|| {
                    OWN.iter()
                        .find(|(act, _, _)| *act == self)
                        .and_then(|(_, _, chord)| chord.parse::<DefaultChord>().ok())
                        .and_then(Shortcut::from_default_chord)
                })
        };
        found.filter(|keys| !keys.keys().is_empty())
    }

    /// Whether a text being edited keeps this chord for itself: undo and redo go to the text first,
    /// as do the chords whose key moves or deletes in it.
    #[must_use]
    pub(crate) fn belongs_to_text(self) -> bool {
        match self {
            Act::Undo
            | Act::Redo
            | Act::DeletePage
            | Act::MovePageEarlier
            | Act::MovePageLater
            | Act::NextSheet
            | Act::PreviousSheet => true,
            Act::File(action) => matches!(action, FileAction::CopyFile | FileAction::MoveToTrash),
            Act::Palette
            | Act::Find
            | Act::FindNext
            | Act::FindPrevious
            | Act::Info
            | Act::Close
            | Act::OpenFile
            | Act::Save
            | Act::ZoomIn
            | Act::ZoomOut
            | Act::ZoomToFit
            | Act::ZoomToActual
            | Act::Done => false,
        }
    }
}

/// The viewer's name in the keymap, or `None` if it does not read (a test rules that out).
pub(crate) fn app() -> Option<AppId> {
    AppId::new(APP).ok()
}

/// Every action the viewer declares with its default chord: its own and those of the file.
pub(crate) fn rows() -> Vec<(AppAction, DefaultChord)> {
    let own = OWN.iter().filter_map(|(_, id, chord)| {
        Some((
            AppAction::new(id).ok()?,
            chord.parse::<DefaultChord>().ok()?,
        ))
    });
    let files = own_keys()
        .into_iter()
        .map(|(_, action, chord)| (action, chord));
    own.chain(files).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chordkit::{Desktop, Keymap, Platform};

    #[test]
    fn every_declared_action_reads_and_is_the_act_it_was_declared_for() {
        let declared = OWN.len() + own_keys().len();
        assert_eq!(rows().len(), declared, "every id and chord reads");
        for (act, id, _) in OWN {
            let action = Action::App(AppAction::new(id).expect("an action id"));
            assert_eq!(Act::of(&action), Some(*act), "{id}");
            assert!(act.shortcut().is_some(), "{id} has keys to draw");
        }
        for (standard, act) in STANDARD {
            assert_eq!(Act::of(&Action::Standard(*standard)), Some(*act));
            assert!(
                act.shortcut().is_some(),
                "{standard:?} is bound on this desktop"
            );
        }
    }

    #[test]
    fn the_declared_chords_are_free_on_every_platform() {
        let app = app().expect("the viewer's name reads");
        let desktops = [Desktop::Ours, Desktop::Kde, Desktop::Gnome, Desktop::Other];
        let platforms = desktops
            .into_iter()
            .map(|desktop| Platform::Linux { desktop })
            .chain([Platform::MacOs, Platform::Windows]);
        for platform in platforms {
            let registered = Keymap::conventional(platform)
                .register(&app, &rows())
                .map_err(|conflict| conflict.to_string());
            assert_eq!(registered, Ok(()), "{platform:?}");
        }
    }
}
