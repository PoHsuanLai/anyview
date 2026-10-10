//! The keys of an action.

use super::FileAction;
use super::spec::{Binding, spec_of};
use chordkit::{Action, AppAction, DefaultChord, StandardAction};
use ds_core::vocab::Shortcut;
use ds_core::word::Word;

/// The shortcut of `action`, or `None` when it has none.
///
/// An action that means what a standard action means (open, copy, print, save a copy) carries
/// that standard action's keys, which the window draws as the keymap really binds them. The rest
/// are the viewer's own: their default chord, which the keymap refuses to register when a standard
/// action or the desktop has it, so a clash is a missing shortcut and never a repurposed one.
#[must_use]
pub fn shortcut(action: FileAction) -> Option<Shortcut> {
    match spec_of(action).binding {
        Binding::Unbound => None,
        Binding::Standard(standard) => Some(Shortcut::standard(standard)),
        Binding::Own(_, chord) => chord
            .parse::<DefaultChord>()
            .ok()
            .and_then(Shortcut::from_default_chord),
    }
}

/// The viewer's own actions that act on a file, each with the default chord it asks the keymap
/// for, to register once per window.
#[must_use]
pub fn own_keys() -> Vec<(FileAction, AppAction, DefaultChord)> {
    FileAction::ALL
        .iter()
        .filter_map(|action| match spec_of(*action).binding {
            Binding::Own(id, chord) => Some((
                *action,
                AppAction::new(id).ok()?,
                chord.parse::<DefaultChord>().ok()?,
            )),
            Binding::Unbound | Binding::Standard(_) => None,
        })
        .collect()
}

/// The file action a keymap action is: the standard action an action means, or the viewer's own
/// action registered for it.
#[must_use]
pub fn file_action_of(action: &Action) -> Option<FileAction> {
    match action {
        Action::Standard(standard) => standard_file_action(*standard),
        Action::App(app) => own_keys()
            .into_iter()
            .find(|(_, own, _)| own == app)
            .map(|(file, _, _)| file),
        _ => None,
    }
}

/// The file action that carries `standard`'s keys.
fn standard_file_action(standard: StandardAction) -> Option<FileAction> {
    FileAction::ALL
        .iter()
        .copied()
        .find(|action| matches!(spec_of(*action).binding, Binding::Standard(s) if s == standard))
}
