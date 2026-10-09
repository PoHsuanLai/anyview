//! A key event as the viewer's `Shortcut`. The viewer's chords are written with ⌘, which is the
//! platform's command key: Control on Linux and Windows, Command on macOS, so both fold to
//! `ShortcutKey::Super` here and the machines see one vocabulary.

use dioxus::html::ModifiersInteraction as _;
use dioxus::prelude::{Key, KeyboardEvent, Modifiers};
use ds::prelude::is_command;
use ds_core::vocab::{Shortcut, ShortcutKey};

/// The named keys the viewer reads, and what each stands for.
const NAMED: &[(Key, ShortcutKey)] = &[
    (Key::Enter, ShortcutKey::Enter),
    (Key::Escape, ShortcutKey::Escape),
    (Key::Tab, ShortcutKey::Tab),
    (Key::Backspace, ShortcutKey::Backspace),
    (Key::ArrowUp, ShortcutKey::Up),
    (Key::ArrowDown, ShortcutKey::Down),
    (Key::ArrowLeft, ShortcutKey::Left),
    (Key::ArrowRight, ShortcutKey::Right),
    (Key::Home, ShortcutKey::Home),
    (Key::End, ShortcutKey::End),
    (Key::Delete, ShortcutKey::Delete),
    (Key::PageUp, ShortcutKey::PageUp),
    (Key::PageDown, ShortcutKey::PageDown),
    (Key::Insert, ShortcutKey::Insert),
    (Key::ContextMenu, ShortcutKey::ContextMenu),
];

/// The key a named or typed `key` stands for, or `None` for a modifier on its own and for keys
/// the viewer has no use for (media keys, the function keys but F10).
fn key_of(key: &Key) -> Option<ShortcutKey> {
    if let Key::Character(text) = key {
        return match text.as_str() {
            " " => Some(ShortcutKey::Space),
            typed => typed
                .chars()
                .next()
                .map(|c| ShortcutKey::Char(c.to_lowercase().next().unwrap_or(c))),
        };
    }
    NAMED
        .iter()
        .find(|(named, _)| named == key)
        .map(|(_, shortcut)| *shortcut)
}

/// The modifiers held, as shortcut keys, in no particular order (a `Shortcut` normalises).
fn modifiers_of(held: Modifiers) -> Vec<ShortcutKey> {
    let mut keys = Vec::new();
    if is_command(held) {
        keys.push(ShortcutKey::Super);
    }
    if held.contains(Modifiers::ALT) {
        keys.push(ShortcutKey::Alt);
    }
    if held.contains(Modifiers::SHIFT) {
        keys.push(ShortcutKey::Shift);
    }
    keys
}

/// What `event` is, as a shortcut; `None` when it is a modifier alone or a key nothing uses.
pub(crate) fn shortcut_of(event: &KeyboardEvent) -> Option<Shortcut> {
    // ⇧F10 is the context-menu key on a keyboard without one.
    if event.key() == Key::F10 && event.modifiers() == Modifiers::SHIFT {
        return Some(Shortcut(vec![ShortcutKey::ContextMenu]));
    }
    let key = key_of(&event.key())?;
    let mut keys = modifiers_of(event.modifiers());
    keys.push(key);
    Some(Shortcut(keys))
}

/// The normalised keys of `event`, empty when it is not one the viewer reads.
pub(crate) fn keys_of(event: &KeyboardEvent) -> Vec<ShortcutKey> {
    shortcut_of(event)
        .map(|shortcut| shortcut.keys())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_modifiers_become_the_viewers_shortcuts() {
        // name, key, modifiers, the keys after normalising
        let cases = [
            (
                "a letter is lower case",
                Key::Character("K".into()),
                Modifiers::empty(),
                vec![ShortcutKey::Char('k')],
            ),
            (
                "control is the command key",
                Key::Character("k".into()),
                Modifiers::CONTROL,
                vec![ShortcutKey::Super, ShortcutKey::Char('k')],
            ),
            (
                "so is meta",
                Key::Character("k".into()),
                Modifiers::META,
                vec![ShortcutKey::Super, ShortcutKey::Char('k')],
            ),
            (
                "so is super, as a window reports the Super key",
                Key::Character("k".into()),
                Modifiers::SUPER,
                vec![ShortcutKey::Super, ShortcutKey::Char('k')],
            ),
            (
                "both together are one",
                Key::Character("k".into()),
                Modifiers::CONTROL | Modifiers::META,
                vec![ShortcutKey::Super, ShortcutKey::Char('k')],
            ),
            (
                "shift is kept",
                Key::ArrowLeft,
                Modifiers::SHIFT,
                vec![ShortcutKey::Shift, ShortcutKey::Left],
            ),
            (
                "space is named",
                Key::Character(" ".into()),
                Modifiers::empty(),
                vec![ShortcutKey::Space],
            ),
            (
                "an arrow",
                Key::ArrowRight,
                Modifiers::empty(),
                vec![ShortcutKey::Right],
            ),
        ];
        for (name, key, held, want) in cases {
            let mut keys = modifiers_of(held);
            keys.push(key_of(&key).unwrap_or_else(|| panic!("{name}: no key")));
            assert_eq!(Shortcut(keys).keys(), want, "{name}");
        }
        assert_eq!(key_of(&Key::Shift), None, "a modifier alone");
    }
}
