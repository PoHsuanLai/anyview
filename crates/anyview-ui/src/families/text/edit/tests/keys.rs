//! Typing, deleting, the arrows and undo, through the surface's own key handling.

#![allow(clippy::unwrap_used)]

use super::support::{Rig, SCALES, start_of};
use ds::prelude::ShortcutKey;

#[test]
fn typing_backspace_delete_and_enter_edit_the_text() {
    for scale in SCALES {
        let mut rig = Rig::open("hello\nworld", false, 4, 400, scale);
        rig.click(0, 5);
        rig.type_text(" there");
        assert_eq!(rig.text(), "hello there\nworld", "scale {scale}");
        rig.press(ShortcutKey::Backspace);
        assert_eq!(rig.text(), "hello ther\nworld");
        rig.press(ShortcutKey::Enter);
        assert_eq!(rig.text(), "hello ther\n\nworld");
        assert_eq!(rig.caret(), start_of("hello ther\n\nworld", 1));
        rig.press(ShortcutKey::Up);
        rig.press(ShortcutKey::End);
        rig.press(ShortcutKey::Delete);
        assert_eq!(
            rig.text(),
            "hello ther\nworld",
            "delete joins the next line"
        );
        rig.press(ShortcutKey::Tab);
        assert_eq!(rig.text(), "hello ther\t\nworld");
    }
}

#[test]
fn arrows_home_end_and_word_moves_walk_the_text() {
    for scale in SCALES {
        let mut rig = Rig::open("alpha beta\ngamma", false, 4, 400, scale);
        rig.click(0, 0);
        rig.press(ShortcutKey::Right);
        assert_eq!(rig.caret(), 1);
        rig.chord(&[ShortcutKey::Ctrl], ShortcutKey::Right);
        assert_eq!(rig.caret(), 5, "scale {scale}: to the end of the word");
        rig.chord(&[ShortcutKey::Ctrl], ShortcutKey::Right);
        assert_eq!(rig.caret(), 10);
        rig.chord(&[ShortcutKey::Ctrl], ShortcutKey::Left);
        assert_eq!(rig.caret(), 6);
        rig.press(ShortcutKey::End);
        assert_eq!(rig.caret(), 10);
        rig.press(ShortcutKey::Down);
        assert_eq!(
            rig.caret(),
            16,
            "down keeps the column, clamped to the short line"
        );
        rig.press(ShortcutKey::Home);
        assert_eq!(rig.caret(), 11);
        rig.press(ShortcutKey::Left);
        assert_eq!(rig.caret(), 10, "left over the break");
        rig.chord(&[ShortcutKey::Ctrl], ShortcutKey::End);
        assert_eq!(rig.caret(), 16);
        rig.chord(&[ShortcutKey::Ctrl], ShortcutKey::Home);
        assert_eq!(rig.caret(), 0);
    }
}

#[test]
fn undo_and_redo_step_through_typing_by_word() {
    for scale in SCALES {
        let mut rig = Rig::open("", false, 3, 400, scale);
        rig.type_text("one two");
        rig.ctrl('z');
        assert_eq!(rig.text(), "one ", "scale {scale}");
        rig.ctrl('z');
        assert_eq!(rig.text(), "one");
        rig.ctrl('z');
        assert_eq!(rig.text(), "");
        rig.chord(
            &[ShortcutKey::Ctrl, ShortcutKey::Shift],
            ShortcutKey::Char('z'),
        );
        assert_eq!(rig.text(), "one");
        rig.ctrl('y');
        assert_eq!(rig.text(), "one ");
    }
}
