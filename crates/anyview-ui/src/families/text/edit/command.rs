//! What a key pressed on the edit surface means: pure, so the table is the whole rule.

use anyview_text::Motion;
use chordkit::{Action, Context, Keymap, StandardAction};
use dioxus::prelude::{Key, Modifiers};
use ds_core::command::resolve;

/// What the editor does for a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    /// Move the caret in the text; with `extend`, drag the selection's end.
    Move {
        by: Motion,
        extend: bool,
    },
    /// Move along what is drawn (wrapped rows), so the surface's geometry decides.
    Visual {
        by: Motion,
        extend: bool,
    },
    Enter,
    Tab,
    Backspace,
    Delete,
    SelectAll,
    Undo,
    Redo,
    /// A page of rows up (`-1`) or down (`1`).
    Page(i32),
}

/// The named keys the editor reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Tab,
    Backspace,
    Delete,
}

const NAMED: &[(Key, Named)] = &[
    (Key::ArrowLeft, Named::Left),
    (Key::ArrowRight, Named::Right),
    (Key::ArrowUp, Named::Up),
    (Key::ArrowDown, Named::Down),
    (Key::Home, Named::Home),
    (Key::End, Named::End),
    (Key::PageUp, Named::PageUp),
    (Key::PageDown, Named::PageDown),
    (Key::Enter, Named::Enter),
    (Key::Tab, Named::Tab),
    (Key::Backspace, Named::Backspace),
    (Key::Delete, Named::Delete),
];

/// The command `key` with `modifiers` means under `keymap`, or `None` for a key the editor leaves
/// alone. What a chord on a letter means (select all, undo, redo) is the keymap's, in a text field;
/// how far an arrow with Ctrl, Alt or Command moves is the text's own until the keymap has text
/// actions for the moves.
pub(super) fn command_of(keymap: &Keymap, key: &Key, modifiers: Modifiers) -> Option<Command> {
    let extend = modifiers.contains(Modifiers::SHIFT);
    let command = modifiers.intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::SUPER);
    if let Key::Character(_) = key {
        return resolve(keymap, key, modifiers, Context::TextEntry).and_then(
            |action| match action {
                Action::Standard(StandardAction::SelectAll) => Some(Command::SelectAll),
                Action::Standard(StandardAction::Undo) => Some(Command::Undo),
                Action::Standard(StandardAction::Redo) => Some(Command::Redo),
                // `Action` is non_exhaustive: an action chordkit adds later is not the editor's.
                Action::Standard(_) | Action::App(_) | _ => None,
            },
        );
    }
    let named = NAMED
        .iter()
        .find(|(known, _)| known == key)
        .map(|(_, named)| *named)?;
    let word = modifiers.intersects(Modifiers::CONTROL | Modifiers::ALT);
    let line = modifiers.intersects(Modifiers::META | Modifiers::SUPER);
    let go = |by| Some(Command::Move { by, extend });
    let see = |by| Some(Command::Visual { by, extend });
    match named {
        Named::Left if word => go(Motion::WordLeft),
        Named::Left if line => see(Motion::LineStart),
        Named::Left => go(Motion::Left),
        Named::Right if word => go(Motion::WordRight),
        Named::Right if line => see(Motion::LineEnd),
        Named::Right => go(Motion::Right),
        Named::Up if line => go(Motion::DocStart),
        Named::Up => see(Motion::Up),
        Named::Down if line => go(Motion::DocEnd),
        Named::Down => see(Motion::Down),
        Named::Home if command => go(Motion::DocStart),
        Named::Home => see(Motion::LineStart),
        Named::End if command => go(Motion::DocEnd),
        Named::End => see(Motion::LineEnd),
        Named::PageUp => Some(Command::Page(-1)),
        Named::PageDown => Some(Command::Page(1)),
        Named::Enter => Some(Command::Enter),
        Named::Tab if !extend => Some(Command::Tab),
        Named::Tab => None,
        Named::Backspace => Some(Command::Backspace),
        Named::Delete => Some(Command::Delete),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chordkit::{Desktop, Platform};

    #[test]
    fn each_key_means_one_command() {
        const NONE: Modifiers = Modifiers::empty();
        const SHIFT: Modifiers = Modifiers::SHIFT;
        const CTRL: Modifiers = Modifiers::CONTROL;
        const META: Modifiers = Modifiers::META;
        const SUPER: Modifiers = Modifiers::SUPER;
        let ours = Keymap::conventional(Platform::Linux {
            desktop: Desktop::Ours,
        });
        let go = |by, extend| Some(Command::Move { by, extend });
        let see = |by, extend| Some(Command::Visual { by, extend });
        let letter = |text: &str| Key::Character(text.to_owned());
        // name, key, modifiers, command
        let cases = [
            ("an arrow", Key::ArrowLeft, NONE, go(Motion::Left, false)),
            (
                "shift extends",
                Key::ArrowRight,
                SHIFT,
                go(Motion::Right, true),
            ),
            (
                "control walks a word",
                Key::ArrowLeft,
                CTRL,
                go(Motion::WordLeft, false),
            ),
            (
                "meta goes to the row's end",
                Key::ArrowRight,
                META,
                see(Motion::LineEnd, false),
            ),
            (
                "up follows the rows",
                Key::ArrowUp,
                NONE,
                see(Motion::Up, false),
            ),
            (
                "meta up is the start of the text",
                Key::ArrowUp,
                META,
                go(Motion::DocStart, false),
            ),
            (
                "home is the row's start",
                Key::Home,
                NONE,
                see(Motion::LineStart, false),
            ),
            (
                "control end is the text's end",
                Key::End,
                CTRL,
                go(Motion::DocEnd, false),
            ),
            ("page down", Key::PageDown, NONE, Some(Command::Page(1))),
            ("enter", Key::Enter, NONE, Some(Command::Enter)),
            ("tab", Key::Tab, NONE, Some(Command::Tab)),
            ("shift tab is left alone", Key::Tab, SHIFT, None),
            ("backspace", Key::Backspace, NONE, Some(Command::Backspace)),
            ("delete", Key::Delete, NONE, Some(Command::Delete)),
            ("select all", letter("a"), SUPER, Some(Command::SelectAll)),
            ("undo", letter("z"), SUPER, Some(Command::Undo)),
            (
                "redo with shift",
                letter("Z"),
                SUPER | SHIFT,
                Some(Command::Redo),
            ),
            ("control is not command here", letter("z"), CTRL, None),
            ("a letter is text, not a command", letter("a"), NONE, None),
            ("an unknown chord", letter("q"), SUPER, None),
            ("a key the editor does not read", Key::F5, NONE, None),
        ];
        for (name, key, modifiers, want) in cases {
            assert_eq!(command_of(&ours, &key, modifiers), want, "{name}");
        }
        // Where Ctrl is the command key, it is, and Ctrl+Y redoes.
        let windows = Keymap::conventional(Platform::Windows);
        assert_eq!(
            command_of(&windows, &letter("y"), CTRL),
            Some(Command::Redo)
        );
        assert_eq!(
            command_of(&windows, &letter("z"), CTRL),
            Some(Command::Undo)
        );
    }
}
