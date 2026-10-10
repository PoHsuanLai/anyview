//! Selecting by Shift, by drag and by repeated click; the boxes drawn for it; the clipboard.

#![allow(clippy::unwrap_used)]

use super::support::{Rig, SCALES, start_of};
use dioxus::prelude::Modifiers;
use ds::prelude::ShortcutKey;
use ds_core::press::PointerButton;
use ds_harness::{Driver, Input, PointerAction, PointerInput};

const TEXT: &str = "one two three\nfour five\nsix";

#[test]
fn shift_arrows_and_shift_word_moves_select_and_draw_one_box_a_line() {
    for scale in SCALES {
        let mut rig = Rig::open(TEXT, false, 4, 400, scale);
        rig.click(0, 4);
        rig.shift(ShortcutKey::Right);
        rig.shift(ShortcutKey::Right);
        assert_eq!(rig.range(), (4, 6), "scale {scale}");
        assert_eq!(rig.selection_boxes(), 1);
        rig.chord(&[ShortcutKey::Ctrl, ShortcutKey::Shift], ShortcutKey::Right);
        assert_eq!(rig.range(), (4, 7));
        rig.shift(ShortcutKey::Down);
        assert_eq!(rig.range().0, 4);
        assert!(rig.range().1 > start_of(TEXT, 1), "{:?}", rig.range());
        assert_eq!(
            rig.selection_boxes(),
            2,
            "the selection runs onto the next line: {}",
            rig.boxes_html()
        );
        rig.shift(ShortcutKey::Down);
        assert_eq!(rig.selection_boxes(), 3);
        rig.press(ShortcutKey::Left);
        assert_eq!(rig.range().0, rig.range().1, "an arrow collapses it");
        assert_eq!(rig.selection_boxes(), 0);
    }
}

#[test]
fn shift_click_extends_from_the_caret_and_a_drag_selects_what_it_crosses() {
    for scale in SCALES {
        let mut rig = Rig::open(TEXT, false, 4, 400, scale);
        rig.click(0, 4);
        let to = rig.at(1, 4);
        let shifted = PointerInput::new(to, PointerAction::Click(PointerButton::Primary));
        rig.harness
            .send(Input::Pointer(shifted.with_mods(Modifiers::SHIFT)));
        rig.settle();
        assert_eq!(
            rig.range(),
            (4, start_of(TEXT, 1) + 4),
            "scale {scale}: shift-click"
        );
        let (from, to) = (rig.at(0, 8), rig.at(2, 3));
        rig.harness.send(Input::drag(from, to, 6));
        rig.settle();
        assert_eq!(
            rig.range(),
            (8, start_of(TEXT, 2) + 3),
            "scale {scale}: drag down"
        );
        let (from, to) = (rig.at(1, 6), rig.at(0, 2));
        rig.harness.send(Input::drag(from, to, 6));
        rig.settle();
        assert_eq!(
            rig.selection(),
            (start_of(TEXT, 1) + 6, 2),
            "scale {scale}: drag up"
        );
    }
}

#[test]
fn a_double_click_selects_a_word_and_a_triple_click_a_line() {
    for scale in SCALES {
        let mut rig = Rig::open(TEXT, false, 4, 400, scale);
        let at = rig.at(0, 5);
        rig.harness.send(Input::click(at));
        rig.harness.send(Input::click(at));
        rig.settle();
        assert_eq!(rig.range(), (4, 7), "scale {scale}: the word");
        rig.harness.send(Input::click(at));
        rig.settle();
        assert_eq!(
            rig.range(),
            (0, start_of(TEXT, 1)),
            "scale {scale}: the line"
        );
    }
}

#[test]
fn select_all_copy_cut_and_paste_go_through_the_clipboard() {
    for scale in SCALES {
        let mut rig = Rig::open(TEXT, false, 4, 400, scale);
        rig.ctrl('a');
        assert_eq!(rig.range(), (0, TEXT.len()));
        rig.ctrl('c');
        assert_eq!(
            rig.harness.clipboard_text().as_deref(),
            Some(TEXT),
            "scale {scale}"
        );
        rig.click(0, 3);
        for _ in 0..4 {
            rig.shift(ShortcutKey::Right);
        }
        rig.ctrl('x');
        assert_eq!(rig.text(), "one three\nfour five\nsix", "scale {scale}");
        assert_eq!(rig.harness.clipboard_text().as_deref(), Some(" two"));
        rig.click(2, 3);
        rig.ctrl('v');
        assert_eq!(rig.text(), "one three\nfour five\nsix two");
        rig.harness.set_clipboard_text("a\r\nb");
        rig.ctrl('v');
        assert_eq!(rig.text(), "one three\nfour five\nsix twoa\nb");
        rig.ctrl('z');
        assert_eq!(rig.text(), "one three\nfour five\nsix two");
    }
}
