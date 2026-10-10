//! A key event as the viewer reads it. The keymap says what a chord means (`ds::keys`): the one
//! that resolves to an action of the viewer's is that action, and a key with no command modifier is
//! the key itself. Which modifier is Command or Ctrl is never decided here.

use crate::keys::{Act, Press, app, rows};
use chordkit::{Context, DefaultChord, Modifier, Platform, PrimaryUse};
use dioxus::html::ModifiersInteraction as _;
use dioxus::prelude::{Key, KeyboardEvent, Modifiers, use_hook};
use ds::prelude::{Keys, use_keys};
use ds_core::command::chord_of;
use ds_core::vocab::{Shortcut, ShortcutKey};

/// The window's keymap, with the viewer's own actions declared on it once, each on its own so a
/// chord the person's system keeps for itself leaves that one action unbound and no other.
pub(super) fn use_viewer_keys() -> Keys {
    let keys = use_keys();
    use_hook(|| {
        if let Some(app) = app() {
            for row in rows() {
                let _unbound = keys.register_actions(&app, std::slice::from_ref(&row));
            }
        }
    });
    keys
}

/// What `event` is to the viewer, or `None` for a modifier alone, a chord nothing here answers and
/// a key it has no use for. `context` is `Context::TextEntry` where a text field has the focus.
pub(super) fn press_of(keys: Keys, event: &KeyboardEvent, context: Context) -> Option<Press> {
    if let Some(act) = keys.action_of(event, context).as_ref().and_then(Act::of) {
        return Some(Press::Act(act));
    }
    plain_key(keys.platform(), &event.key(), event.modifiers()).map(Press::Key)
}

/// The key pressed with at most Shift held: a chord with Ctrl, Alt or Command is the keymap's or
/// nobody's. The menu key, and Shift+F10 on a keyboard without one, are the context menu.
fn plain_key(platform: Platform, key: &Key, held: Modifiers) -> Option<Shortcut> {
    if *key == Key::ContextMenu || (*key == Key::F10 && held == Modifiers::SHIFT) {
        return Some(Shortcut(vec![ShortcutKey::ContextMenu]));
    }
    let chord = chord_of(platform, key, held)?;
    let command = [
        Modifier::Ctrl,
        Modifier::Alt,
        Modifier::Super,
        Modifier::Meta,
    ];
    if command
        .iter()
        .any(|modifier| chord.modifiers().contains(*modifier))
    {
        return None;
    }
    Shortcut::from_default_chord(DefaultChord::new(PrimaryUse::Unused, chord))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chordkit::Desktop;

    fn ours() -> Platform {
        Platform::Linux {
            desktop: Desktop::Ours,
        }
    }

    #[test]
    fn a_key_is_plain_unless_a_command_modifier_is_held() {
        let letter = |text: &str| Key::Character(text.to_owned());
        // name, platform, key, modifiers, the keys read
        let cases = [
            (
                "a letter is lower case",
                ours(),
                letter("K"),
                Modifiers::empty(),
                Some(vec![ShortcutKey::Char('k')]),
            ),
            (
                "space is named",
                ours(),
                letter(" "),
                Modifiers::empty(),
                Some(vec![ShortcutKey::Space]),
            ),
            (
                "shift is kept",
                ours(),
                Key::ArrowLeft,
                Modifiers::SHIFT,
                Some(vec![ShortcutKey::Shift, ShortcutKey::Left]),
            ),
            (
                "shift and equals is plus",
                ours(),
                letter("="),
                Modifiers::SHIFT,
                Some(vec![ShortcutKey::Char('+')]),
            ),
            (
                "the menu key",
                ours(),
                Key::ContextMenu,
                Modifiers::empty(),
                Some(vec![ShortcutKey::ContextMenu]),
            ),
            (
                "shift f10 is the menu key",
                ours(),
                Key::F10,
                Modifiers::SHIFT,
                Some(vec![ShortcutKey::ContextMenu]),
            ),
            (
                "command is the keymap's",
                ours(),
                letter("k"),
                Modifiers::SUPER,
                None,
            ),
            (
                "so is control",
                Platform::Windows,
                letter("k"),
                Modifiers::CONTROL,
                None,
            ),
            (
                "a modifier alone is no key",
                ours(),
                Key::Shift,
                Modifiers::SHIFT,
                None,
            ),
        ];
        for (name, platform, key, held, want) in cases {
            let got = plain_key(platform, &key, held).map(|shortcut| shortcut.keys());
            assert_eq!(got, want, "{name}");
        }
    }
}
