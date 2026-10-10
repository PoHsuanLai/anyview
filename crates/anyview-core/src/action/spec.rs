//! What is true of each action regardless of the file: where it may appear and which keys it has.
//! The one match on `FileAction`.

use super::{FileAction, Reach};
use ds_core::standard_action::StandardAction;

/// How an action is bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Binding {
    /// No shortcut.
    Unbound,
    /// A combination the Mac reserves for this meaning (copy, print…): the only way to bind one.
    Standard(StandardAction),
    /// An action of the viewer's own: its id in the keymap and its default chord, written in
    /// chordkit's text (`Primary` is Command or Ctrl, whichever the platform means). Registering
    /// it refuses a chord a standard action or the desktop has.
    Own(&'static str, &'static str),
}

/// An action's reach and its keys.
#[derive(Debug, Clone, Copy)]
pub(super) struct ActionSpec {
    pub(super) reach: Reach,
    pub(super) binding: Binding,
}

const fn spec(reach: Reach, binding: Binding) -> ActionSpec {
    ActionSpec { reach, binding }
}

pub(super) fn spec_of(action: FileAction) -> ActionSpec {
    match action {
        FileAction::Open => spec(Reach::Launcher, Binding::Standard(StandardAction::Open)),
        FileAction::RevealInFolder => spec(Reach::Both, Binding::Standard(StandardAction::Reveal)),
        FileAction::CopyFile => spec(Reach::Both, Binding::Standard(StandardAction::Copy)),
        FileAction::CopyPath => spec(
            Reach::Both,
            Binding::Own("anyview.copy-path", "Primary+Alt+C"),
        ),
        FileAction::Share => spec(Reach::Both, Binding::Unbound),
        FileAction::Rename => spec(Reach::Both, Binding::Unbound),
        FileAction::Duplicate => spec(Reach::Both, Binding::Own("anyview.duplicate", "Primary+D")),
        FileAction::MoveToTrash => spec(
            Reach::Both,
            Binding::Own("anyview.move-to-trash", "Primary+Backspace"),
        ),
        FileAction::Print => spec(Reach::Viewer, Binding::Standard(StandardAction::Print)),
        FileAction::Export => spec(
            Reach::Viewer,
            Binding::Own("anyview.export", "Primary+Shift+E"),
        ),
        FileAction::SaveCopy => spec(Reach::Viewer, Binding::Standard(StandardAction::SaveAs)),
        FileAction::RevertTo => spec(Reach::Viewer, Binding::Unbound),
        FileAction::RotateLeft => spec(
            Reach::Viewer,
            Binding::Own("anyview.rotate-left", "Primary+["),
        ),
        FileAction::RotateRight => spec(
            Reach::Viewer,
            Binding::Own("anyview.rotate-right", "Primary+]"),
        ),
        FileAction::FlipHorizontal => spec(Reach::Viewer, Binding::Unbound),
        FileAction::FlipVertical => spec(Reach::Viewer, Binding::Unbound),
        FileAction::PlayInBackground => spec(Reach::Both, Binding::Unbound),
        FileAction::PlayInMiniWindow => spec(Reach::Both, Binding::Unbound),
        FileAction::ConvertTo => spec(Reach::Both, Binding::Unbound),
    }
}
