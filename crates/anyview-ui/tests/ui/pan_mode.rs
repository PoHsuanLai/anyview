//! Panning a picture is a mode, as in Preview, Photos and Loupe: a drag moves a zoomed picture
//! under Pan (the default) and not under Select, the wheel moves it whatever the mode, the cursor is
//! the hand only in Pan, and the Select | Pan control sits on the titlebar's trailing side.

use crate::support;

use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use support::{folder, settle};

const SCALES: [u16; 2] = [100, 200];

fn zoomed(scale: u16) -> (tempfile::TempDir, Harness) {
    let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let wiring = support::Wiring {
        viewport: Some(ds_harness::Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        record_moves: true,
        ..support::Wiring::default()
    };
    let (mut harness, _, _) = support::wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    harness.send(Input::pointer_move(point(450.0, 300.0)));
    for _ in 0..8 {
        harness.send(Input::key(ShortcutKey::Char('+')));
        settle(&mut harness);
    }
    (dir, harness)
}

fn point(x: f32, y: f32) -> Point {
    Point { x: Px(x), y: Px(y) }
}

fn placed(harness: &Harness) -> String {
    harness
        .attr(".viewer-raster-picture", "style")
        .unwrap_or_default()
}

fn mode(harness: &Harness) -> Option<String> {
    harness.attr(".viewer-raster", "data-pan")
}

fn drag(harness: &mut Harness) {
    // Blitz judges a double-click by the wall clock, and a double-click zooms: keep drags apart.
    std::thread::sleep(std::time::Duration::from_millis(700));
    harness.send(Input::drag(point(450.0, 300.0), point(400.0, 270.0), 5));
    settle(harness);
}

const SEGMENTS: &str = ".ds-titlebar-trailing .ds-segmented-segment";

fn choose(harness: &mut Harness, which: usize) {
    let at = harness
        .centre(&format!("{SEGMENTS}:nth-child({which})"))
        .unwrap();
    harness.send(Input::click(at));
    settle(harness);
    harness.send(Input::pointer_move(point(450.0, 300.0)));
}

#[test]
fn a_drag_pans_a_zoomed_picture_by_default_and_select_makes_it_inert() {
    for scale in SCALES {
        let (_dir, mut harness) = zoomed(scale);
        assert_eq!(
            mode(&harness).as_deref(),
            Some("on"),
            "{scale}: starts in Pan"
        );
        let before = placed(&harness);
        drag(&mut harness);
        let panned = placed(&harness);
        assert_ne!(panned, before, "{scale}: a drag in Pan moves it");
        harness.send(Input::key(ShortcutKey::Char('h')));
        settle(&mut harness);
        assert_eq!(
            mode(&harness).as_deref(),
            Some("off"),
            "{scale}: H switches to Select"
        );
        drag(&mut harness);
        assert_eq!(
            placed(&harness),
            panned,
            "{scale}: a drag in Select does not pan"
        );
        harness.send(Input::key(ShortcutKey::Char('h')));
        settle(&mut harness);
        assert_eq!(
            mode(&harness).as_deref(),
            Some("on"),
            "{scale}: H switches back"
        );
        drag(&mut harness);
        assert_ne!(placed(&harness), panned, "{scale}: and a drag pans again");
    }
}

#[test]
fn the_wheel_pans_whatever_the_mode() {
    for scale in SCALES {
        let (_dir, mut harness) = zoomed(scale);
        harness.send(Input::key(ShortcutKey::Char('h')));
        settle(&mut harness);
        assert_eq!(mode(&harness).as_deref(), Some("off"), "{scale}");
        let before = placed(&harness);
        harness.send(Input::wheel(point(450.0, 300.0), Px(0.0), Px(-40.0)));
        settle(&mut harness);
        assert_ne!(
            placed(&harness),
            before,
            "{scale}: the wheel moved the picture in Select"
        );
    }
}

#[test]
fn the_segmented_control_is_on_the_titlebars_trailing_side_and_switches_the_mode() {
    for scale in SCALES {
        let (_dir, mut harness) = zoomed(scale);
        assert_eq!(
            harness.count(".viewer-mode"),
            0,
            "{scale}: nothing floats over the picture"
        );
        assert_eq!(
            harness.count(".viewer-titlebar .ds-titlebar-trailing"),
            1,
            "{scale}: the control is in the titlebar"
        );
        assert_eq!(harness.count(SEGMENTS), 2, "{scale}: Select and Pan");
        let bar = harness.rect(".viewer-titlebar").unwrap();
        let control = harness.rect(".ds-titlebar-trailing").unwrap();
        let window = harness.rect(".viewer").unwrap();
        assert!(
            control.origin.x.0 > window.origin.x.0 + window.size.width.0 / 2.0,
            "{scale}: on the trailing half of the bar"
        );
        assert!(
            control.origin.y.0 >= bar.origin.y.0
                && control.origin.y.0 + control.size.height.0
                    <= bar.origin.y.0 + bar.size.height.0 + 1.0,
            "{scale}: inside the bar's height"
        );
        choose(&mut harness, 1);
        assert_eq!(mode(&harness).as_deref(), Some("off"), "{scale}: Select");
        choose(&mut harness, 2);
        assert_eq!(mode(&harness).as_deref(), Some("on"), "{scale}: Pan");
        assert_eq!(
            support::window_moves(),
            0,
            "{scale}: clicking a segment does not move the window"
        );
        // A press that travels over the control is the control's own, not a window move.
        let at = harness.centre(&format!("{SEGMENTS}:nth-child(1)")).unwrap();
        harness.send(Input::drag(at, point(at.x.0 - 40.0, at.y.0 + 3.0), 5));
        settle(&mut harness);
        assert_eq!(
            support::window_moves(),
            0,
            "{scale}: a drag over the control begins no window move"
        );
    }
}

#[test]
fn the_palette_row_names_the_tool_it_switches_to() {
    for scale in SCALES {
        let (_dir, mut harness) = zoomed(scale);
        let open = |harness: &mut Harness| {
            harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
            settle(harness);
            harness.html()
        };
        let html = open(&mut harness);
        assert!(
            html.contains("Use Select"),
            "{scale}: in Pan, the row is Use Select"
        );
        assert!(!html.contains("Use Pan"), "{scale}");
        harness.send(Input::key(ShortcutKey::Escape));
        settle(&mut harness);
        harness.send(Input::key(ShortcutKey::Char('h')));
        settle(&mut harness);
        let html = open(&mut harness);
        assert!(
            html.contains("Use Pan"),
            "{scale}: in Select, the row is Use Pan"
        );
        assert!(!html.contains("Use Select"), "{scale}");
    }
}
