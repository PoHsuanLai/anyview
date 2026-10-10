//! The IME: a preedit drawn underlined at the caret, then committed; CJK text; the candidate
//! window's area.

#![allow(clippy::unwrap_used)]

use super::support::{Rig, SCALES, near};
use ds::prelude::ShortcutKey;
use ds_harness::{Driver, Input, Query};

#[test]
fn a_zhuyin_composition_shows_underlined_at_the_caret_then_commits() {
    for scale in SCALES {
        for wrap in [false, true] {
            let mut rig = Rig::open("ab", wrap, 3, 400, scale);
            rig.click(0, 1);
            rig.harness.send(Input::ime_start());
            rig.harness.send(Input::ime_update("ㄓ", 3));
            rig.settle();
            assert_eq!(rig.text(), "ab", "the preedit is not in the text yet");
            assert_eq!(rig.preedit().as_deref(), Some("ㄓ"));
            assert_eq!(
                rig.harness.text_of(".viewer-preedit").as_deref(),
                Some("ㄓ"),
                "scale {scale}"
            );
            assert_eq!(rig.harness.text_of(".viewer-code").as_deref(), Some("aㄓb"));
            let before = rig.drawn_caret().unwrap();
            rig.harness.send(Input::ime_update("ㄓㄨ", 6));
            rig.settle();
            let after = rig.drawn_caret().unwrap();
            assert!(
                after.origin.x.0 > before.origin.x.0 + 4.0,
                "the caret moves with the preedit"
            );
            let underlined = rig.harness.rect(".viewer-preedit").unwrap();
            assert!(underlined.size.width.0 > 8.0, "{underlined:?}");
            rig.harness.send(Input::ime_commit("注"));
            rig.settle();
            assert_eq!(rig.text(), "a注b", "scale {scale} wrap {wrap}");
            assert_eq!(rig.caret(), 1 + "注".len());
            assert_eq!(rig.harness.count(".viewer-preedit"), 0);
            assert_eq!(rig.preedit(), None);
        }
    }
}

#[test]
fn a_commit_replaces_a_selection_and_one_undo_takes_back_the_whole_word() {
    for scale in SCALES {
        let mut rig = Rig::open("abcdef", false, 3, 400, scale);
        rig.click(0, 1);
        rig.shift(ShortcutKey::Right);
        rig.shift(ShortcutKey::Right);
        rig.harness.send(Input::ime_update("ni", 2));
        rig.settle();
        assert_eq!(
            rig.text(),
            "adef",
            "the first preedit took the selection out"
        );
        rig.harness.send(Input::ime_commit("你好"));
        rig.settle();
        assert_eq!(rig.text(), "a你好def", "scale {scale}");
        rig.command('z');
        assert_eq!(rig.text(), "adef");
    }
}

#[test]
fn a_cancelled_composition_leaves_the_text_alone_and_typing_goes_on() {
    for scale in SCALES {
        let mut rig = Rig::open("x", false, 3, 400, scale);
        rig.click(0, 1);
        rig.harness.send(Input::ime_update("か", 3));
        rig.settle();
        rig.harness.send(Input::ime_update("", 0));
        rig.harness.send(Input::ime_end());
        rig.settle();
        assert_eq!(rig.text(), "x");
        assert_eq!(rig.preedit(), None);
        rig.type_text("y");
        assert_eq!(rig.text(), "xy");
    }
}

#[test]
fn clicking_a_line_with_a_cjk_run_and_committing_cjk_text_keeps_the_caret_honest() {
    for scale in SCALES {
        let mut rig = Rig::open("日本語\nenglish", false, 3, 400, scale);
        rig.click(0, 6);
        rig.harness.send(Input::ime_update("にほん", 9));
        rig.harness.send(Input::ime_commit("日本"));
        rig.settle();
        assert_eq!(rig.text(), "日本日本語\nenglish", "scale {scale}");
        let caret = rig.drawn_caret().unwrap();
        let wanted = rig.rect_of(0, 12);
        assert!(
            near(caret.origin.x.0, wanted.origin.x.0),
            "{caret:?} {wanted:?}"
        );
    }
}

#[test]
fn the_candidate_window_sits_at_the_caret_and_follows_it() {
    for scale in SCALES {
        let mut rig = Rig::open("hello", false, 3, 400, scale);
        assert!(
            rig.harness.ime_cursor_area().is_some(),
            "scale {scale}: the click took the IME"
        );
        rig.click(0, 1);
        let one = rig.harness.ime_cursor_area().unwrap();
        let caret = rig.rect_of(0, 1);
        assert!(near(one.origin.x.0, caret.origin.x.0), "{one:?} {caret:?}");
        assert!(near(one.origin.y.0, caret.origin.y.0), "{one:?} {caret:?}");
        rig.click(0, 4);
        let four = rig.harness.ime_cursor_area().unwrap();
        assert!(four.origin.x.0 > one.origin.x.0 + 10.0, "{four:?}");
    }
}
