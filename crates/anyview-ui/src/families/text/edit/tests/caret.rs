//! A click lands the caret, the caret is drawn where the surface lays the text out, and the
//! room follows it.

#![allow(clippy::unwrap_used)]

use super::support::{Rig, SAMPLE, SCALES, near, start_of};
use ds::prelude::ShortcutKey;
use ds_harness::Driver;

#[test]
fn a_click_on_the_text_puts_the_caret_there_and_draws_it_there() {
    for scale in SCALES {
        for wrap in [false, true] {
            let mut rig = Rig::open(SAMPLE, wrap, 8, 500, scale);
            for (line, col) in [(0, 3), (1, 8), (1, 20), (2, 1), (4, 5)] {
                rig.click(line, col);
                let want = start_of(SAMPLE, line) + col;
                assert_eq!(
                    rig.caret(),
                    want,
                    "scale {scale} wrap {wrap} ({line},{col})"
                );
                let wanted = rig.rect_of(line, col);
                let drawn = rig.drawn_caret().unwrap();
                assert!(
                    near(drawn.origin.x.0, wanted.origin.x.0),
                    "{drawn:?} {wanted:?}"
                );
                assert!(
                    near(drawn.origin.y.0, wanted.origin.y.0),
                    "{drawn:?} {wanted:?}"
                );
                assert!(drawn.size.height.0 > 8.0, "{drawn:?}");
            }
        }
    }
}

#[test]
fn a_click_past_the_end_of_a_line_or_on_an_empty_one_lands_at_its_end() {
    for scale in SCALES {
        let mut rig = Rig::open(SAMPLE, false, 8, 500, scale);
        let far = rig.rect_of(4, 5).origin;
        let row = rig.rect_of(4, 0).size.height.0;
        rig.harness
            .send(ds_harness::Input::click(ds::prelude::Point {
                x: ds::prelude::Px(far.x.0 + 120.0),
                y: ds::prelude::Px(far.y.0 + row / 2.0),
            }));
        rig.settle();
        assert_eq!(
            rig.caret(),
            start_of(SAMPLE, 4) + 5,
            "scale {scale}: past the end"
        );
        rig.click(3, 0);
        assert_eq!(
            rig.caret(),
            start_of(SAMPLE, 3),
            "scale {scale}: the empty line"
        );
        rig.click(5, 0);
        assert_eq!(
            rig.caret(),
            SAMPLE.len(),
            "scale {scale}: the line after the last break"
        );
    }
}

#[test]
fn a_click_in_cjk_text_lands_between_characters() {
    let text = "日本語のテキスト\nabc";
    for scale in SCALES {
        let mut rig = Rig::open(text, false, 4, 500, scale);
        for col in [3, 9, 15, 24] {
            rig.click(0, col);
            assert_eq!(rig.caret(), col, "scale {scale}");
        }
    }
}

#[test]
fn a_line_made_of_several_text_nodes_counts_one_offset_through_them() {
    // The room draws a space-run and a word-run as separate text nodes, as highlighting does.
    let text = "a  bb   ccc    dddd";
    for scale in SCALES {
        let mut rig = Rig::open(text, false, 2, 500, scale);
        for col in [1, 3, 5, 8, 11, 15, 19] {
            rig.click(0, col);
            assert_eq!(rig.caret(), col, "scale {scale}");
        }
    }
}

#[test]
fn the_drawn_caret_follows_typing_and_returns_with_backspace() {
    for scale in SCALES {
        let mut rig = Rig::open("ab", false, 2, 400, scale);
        rig.click(0, 2);
        let before = rig.drawn_caret().unwrap();
        rig.type_text("cd");
        let after = rig.drawn_caret().unwrap();
        assert!(
            after.origin.x.0 > before.origin.x.0 + 4.0,
            "{before:?} {after:?}"
        );
        assert!(near(after.origin.y.0, before.origin.y.0));
        rig.press(ShortcutKey::Backspace);
        rig.press(ShortcutKey::Backspace);
        let back = rig.drawn_caret().unwrap();
        assert!(
            near(back.origin.x.0, before.origin.x.0),
            "{back:?} {before:?}"
        );
    }
}

#[test]
fn in_a_wrapped_line_a_click_on_the_second_row_is_further_into_the_line() {
    let text = "the quick brown fox jumps over the lazy dog and keeps running past the end";
    for scale in SCALES {
        let mut rig = Rig::open(text, true, 4, 260, scale);
        let first = rig.rect_of(0, 0);
        let last = rig.rect_of(0, text.len());
        assert!(
            last.origin.y.0 > first.origin.y.0 + 1.0,
            "it wraps: {first:?} {last:?}"
        );
        let rows = ((last.origin.y.0 - first.origin.y.0) / first.size.height.0).round() as usize;
        assert!(rows >= 1, "scale {scale}");
        // A click on the middle of the last row, at its start: the offset where that row begins.
        let second_row_start = (1..text.len())
            .find(|c| rig.rect_of(0, *c).origin.y.0 > first.origin.y.0 + 1.0)
            .unwrap();
        rig.click(0, second_row_start + 2);
        assert_eq!(rig.caret(), second_row_start + 2, "scale {scale}");
    }
}

#[test]
fn up_and_down_follow_wrapped_rows_and_home_and_end_cut_a_row() {
    let text = "the quick brown fox jumps over the lazy dog and keeps running past the end";
    for scale in SCALES {
        let mut rig = Rig::open(text, true, 4, 260, scale);
        let first_y = rig.rect_of(0, 0).origin.y.0;
        let second_row = (1..text.len())
            .find(|c| rig.rect_of(0, *c).origin.y.0 > first_y + 1.0)
            .unwrap();
        rig.click(0, 4);
        rig.press(ShortcutKey::Down);
        let moved = rig.caret();
        assert!(
            moved >= second_row && moved < text.len(),
            "scale {scale}: down {moved}"
        );
        let drawn = rig.drawn_caret().unwrap();
        assert!(drawn.origin.y.0 > first_y + 1.0, "{drawn:?}");
        rig.press(ShortcutKey::Up);
        assert_eq!(rig.caret(), 4, "scale {scale}: up returns to the column");
        rig.press(ShortcutKey::End);
        assert_eq!(
            rig.caret(),
            second_row,
            "scale {scale}: End goes to the wrap point"
        );
        // The surface draws a position at the start of the row it begins; the row End was
        // pressed on gets its caret from the editor itself, at its last character's edge.
        let drawn = rig.drawn_caret().unwrap();
        assert!(
            near(drawn.origin.y.0, first_y),
            "scale {scale}: on the first row {drawn:?}"
        );
        let last = rig.rect_of(0, second_row - 1);
        assert!(
            drawn.origin.x.0 > last.origin.x.0 + 3.0,
            "right of the last glyph {drawn:?} {last:?}"
        );
        rig.press(ShortcutKey::Right);
        let next = rig.drawn_caret().unwrap();
        assert!(
            next.origin.y.0 > first_y + 1.0,
            "moving on leaves the row: {next:?}"
        );
        rig.click(0, second_row + 3);
        rig.press(ShortcutKey::Home);
        assert_eq!(
            rig.caret(),
            second_row,
            "scale {scale}: Home goes to the row's start"
        );
    }
}

#[test]
fn the_room_scrolls_to_keep_the_caret_in_view() {
    let text: String = (0..30).map(|n| format!("row {n}\n")).collect();
    for scale in SCALES {
        let mut rig = Rig::open(&text, false, 5, 400, scale);
        assert_eq!(rig.first(), 0);
        for _ in 0..7 {
            rig.press(ShortcutKey::Down);
        }
        assert_eq!(rig.first(), 3, "scale {scale}");
        assert!(rig.drawn_caret().is_some());
        rig.chord(&[ShortcutKey::Ctrl], ShortcutKey::Home);
        assert_eq!(rig.first(), 0);
        rig.press(ShortcutKey::PageDown);
        assert_eq!(rig.first(), 1, "a page down moves the caret five lines");
        assert_eq!(rig.caret(), start_of(&text, 5));
    }
}
