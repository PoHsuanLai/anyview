//! What is true of each action regardless of the file: where it may appear and which keys it has.
//! The one match on `FileAction`.

use super::{FileAction, Reach};
use ds_core::standard_action::StandardAction;
use ds_core::vocab::ShortcutKey;

/// How an action is bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Binding {
    /// No shortcut.
    Unbound,
    /// A combination the Mac reserves for this meaning (copy, print…): the only way to bind one.
    Standard(StandardAction),
    /// A combination of the viewer's own. `Shortcut::custom` refuses one the standard table
    /// reserves.
    Own(&'static [ShortcutKey]),
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
    use ShortcutKey::{Alt, Backspace, Char, Shift, Super};
    match action {
        FileAction::Open => spec(Reach::Launcher, Binding::Standard(StandardAction::Open)),
        FileAction::RevealInFolder => spec(Reach::Both, Binding::Standard(StandardAction::Reveal)),
        FileAction::CopyFile => spec(Reach::Both, Binding::Standard(StandardAction::Copy)),
        FileAction::CopyPath => spec(Reach::Both, Binding::Own(&[Alt, Super, Char('c')])),
        FileAction::Share => spec(Reach::Both, Binding::Unbound),
        FileAction::Rename => spec(Reach::Both, Binding::Unbound),
        FileAction::Duplicate => spec(Reach::Both, Binding::Own(&[Super, Char('d')])),
        FileAction::MoveToTrash => spec(Reach::Both, Binding::Own(&[Super, Backspace])),
        FileAction::Print => spec(Reach::Viewer, Binding::Standard(StandardAction::Print)),
        FileAction::Export => spec(Reach::Viewer, Binding::Own(&[Shift, Super, Char('e')])),
        FileAction::SaveCopy => spec(Reach::Viewer, Binding::Standard(StandardAction::SaveAs)),
        FileAction::RevertTo => spec(Reach::Viewer, Binding::Unbound),
        FileAction::RotateLeft => spec(Reach::Viewer, Binding::Own(&[Super, Char('[')])),
        FileAction::RotateRight => spec(Reach::Viewer, Binding::Own(&[Super, Char(']')])),
        FileAction::FlipHorizontal => spec(Reach::Viewer, Binding::Unbound),
        FileAction::FlipVertical => spec(Reach::Viewer, Binding::Unbound),
        FileAction::PlayInBackground => spec(Reach::Both, Binding::Unbound),
        FileAction::PlayInMiniWindow => spec(Reach::Both, Binding::Unbound),
        FileAction::ConvertTo => spec(Reach::Both, Binding::Unbound),
    }
}
