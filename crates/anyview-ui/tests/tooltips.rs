//! The hover tips of the controls: a button's `title` is a tooltip that shows as soon as the
//! pointer is over it (quire's default), goes when the pointer leaves, and stays inside the window.

#![allow(clippy::unwrap_used)]

mod support;

use ds::prelude::{Appearance, Point, Px, Rect, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::time::Duration;
use support::{Wiring, settle, wired};

const SCALES: [u16; 2] = [100, 200];
const TIP: &str = ".ds-tooltip";

fn opened(width: u32, height: u32, scale_percent: u16) -> (Harness, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = support::image_file(dir.path(), "p.png", "quadrants.png");
    let wiring = Wiring {
        viewport: Some(Viewport {
            width,
            height,
            scale_percent,
        }),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    // The chrome is hidden until the pointer moves over the window.
    harness.send(Input::pointer_move(Point {
        x: Px(width as f32 / 2.0),
        y: Px(height as f32 / 3.0),
    }));
    settle(&mut harness);
    (harness, dir)
}

fn hover(harness: &mut Harness, label: &str) {
    let at = harness
        .centre(&format!("[aria-label=\"{label}\"]"))
        .unwrap_or_else(|| panic!("no {label} button"));
    harness.send(Input::pointer_move(at));
}

fn on_screen(rect: Rect, width: u32, height: u32) -> bool {
    rect.origin.x.0 >= 0.0
        && rect.origin.y.0 >= 0.0
        && rect.origin.x.0 + rect.size.width.0 <= width as f32 + 0.5
        && rect.origin.y.0 + rect.size.height.0 <= height as f32 + 0.5
}

#[test]
fn a_capsule_button_shows_its_name_as_soon_as_the_pointer_is_over_it() {
    for scale in SCALES {
        let (mut harness, _dir) = opened(900, 600, scale);
        hover(&mut harness, "Zoom in");
        settle(&mut harness);
        assert_eq!(harness.count(TIP), 1, "{scale}%: no tip under the pointer");
        assert_eq!(harness.text_of(TIP).as_deref(), Some("Zoom in"), "{scale}%");
    }
}

#[test]
fn each_capsule_button_says_its_own_words() {
    let (mut harness, _dir) = opened(900, 600, 100);
    for label in ["Zoom out", "Zoom in", "Rotate left", "Rotate right"] {
        hover(&mut harness, label);
        harness.advance(Duration::from_millis(1300));
        assert_eq!(harness.text_of(TIP).as_deref(), Some(label));
    }
}

#[test]
fn moving_away_hides_the_tip() {
    for scale in SCALES {
        let (mut harness, _dir) = opened(900, 600, scale);
        hover(&mut harness, "Zoom in");
        harness.advance(Duration::from_millis(1300));
        assert_eq!(harness.count(TIP), 1);
        harness.send(Input::pointer_move(Point {
            x: Px(450.0),
            y: Px(200.0),
        }));
        harness.advance(Duration::from_millis(600));
        assert_eq!(
            harness.count(TIP),
            0,
            "{scale}%: the tip outstays the pointer"
        );
    }
}

#[test]
fn a_tip_stays_inside_a_small_window() {
    for scale in SCALES {
        for (width, height) in [(320, 240), (260, 200)] {
            let (mut harness, _dir) = opened(width, height, scale);
            let mut seen = 0;
            for label in ["Zoom out", "Zoom in", "Rotate right"] {
                let sel = format!("[aria-label=\"{label}\"]");
                if harness.rect(&sel).is_none_or(|r| r.size.width.0 <= 0.0) {
                    continue;
                }
                seen += 1;
                hover(&mut harness, label);
                harness.advance(Duration::from_millis(1300));
                let rect = harness.rect(TIP).expect("the tip is drawn");
                assert!(
                    on_screen(rect, width, height),
                    "{label} at {scale}% in {width}x{height}: tip {rect:?} leaves the window"
                );
            }
            assert!(seen > 0, "{scale}% in {width}x{height}: no button to hover");
        }
    }
}

#[test]
fn the_find_bar_buttons_have_tips() {
    let dir = tempfile::tempdir().unwrap();
    let path = support::text_file(dir.path(), "notes.txt", "word", 200);
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), Wiring::default());
    settle(&mut harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('f')));
    settle(&mut harness);
    for label in ["Previous match", "Next match", "Close find"] {
        hover(&mut harness, label);
        harness.advance(Duration::from_millis(1300));
        assert_eq!(harness.text_of(TIP).as_deref(), Some(label));
    }
}
