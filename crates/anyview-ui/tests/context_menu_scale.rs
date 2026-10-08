//! The right-click menu where a person really uses it: over the picture itself, at the scales a
//! desktop runs at, with a hand that moves a little, near the window's edges.

#![allow(clippy::unwrap_used)]

mod support;

use ds::prelude::{Appearance, Point, Px, Rect, ShortcutKey};
use ds_core::press::PointerButton;
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use support::{Wiring, settle, wired};

const SCALES: [u16; 3] = [100, 150, 200];
const WIDTH: u32 = 900;
const HEIGHT: u32 = 600;

fn at(x: f32, y: f32) -> Point {
    Point { x: Px(x), y: Px(y) }
}

fn opened(path: PathBuf, scale_percent: u16) -> (Harness, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let wiring = Wiring {
        viewport: Some(Viewport {
            width: WIDTH,
            height: HEIGHT,
            scale_percent,
        }),
        ..Wiring::default()
    };
    let path = std::fs::canonicalize(path).unwrap();
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    (harness, dir)
}

/// The menu a secondary press at `from` that moves `drift` logical px before the release opens.
fn right_click(harness: &mut Harness, from: Point, drift: f32) -> Option<Rect> {
    let to = at(from.x.0 + drift, from.y.0 + drift);
    harness.send(Input::pointer_move(from));
    harness.send(Input::button_down(from, PointerButton::Secondary));
    if drift != 0.0 {
        harness.send(Input::pointer_move(to));
    }
    harness.send(Input::button_up(to, PointerButton::Secondary));
    settle(harness);
    harness.rect(".ds-menu")
}

fn text(dir: &std::path::Path) -> PathBuf {
    support::text_file(dir, "notes.txt", "word", 200)
}

fn on_screen(rect: Rect) -> bool {
    rect.origin.x.0 >= 0.0
        && rect.origin.y.0 >= 0.0
        && rect.origin.x.0 + rect.size.width.0 <= WIDTH as f32
        && rect.origin.y.0 + rect.size.height.0 <= HEIGHT as f32
}

/// The middle of the picture: a texture layer is a custom widget, and Blitz hands a widget its
/// pointer events and returns before it makes the click and the contextmenu out of a release
/// (blitz-dom events/mod.rs, "Handle event forwarding for custom widget"), so no menu comes.
fn over_the_picture(name: &str, fixture: &str) {
    for scale in SCALES {
        let (mut harness, _dir) = opened(support::fixture("anyview-image", fixture), scale);
        let picture = harness.rect(".viewer-raster-picture").unwrap();
        let middle = at(
            picture.origin.x.0 + picture.size.width.0 / 2.0,
            picture.origin.y.0 + picture.size.height.0 / 2.0,
        );
        let menu = right_click(&mut harness, middle, 0.0);
        assert!(
            menu.is_some(),
            "{name} at {scale}%: a right-click at {middle:?}, inside the picture {picture:?}, opens no menu"
        );
    }
}

#[test]
fn a_right_click_over_a_png_opens_the_menu() {
    over_the_picture("png", "quadrants.png");
}

#[test]
fn a_right_click_over_a_gif_opens_the_menu() {
    over_the_picture("gif", "spin.gif");
}

#[test]
fn a_right_click_over_an_svg_opens_the_menu() {
    over_the_picture("svg", "logo.svg");
}

#[test]
fn a_right_click_beside_the_picture_opens_the_menu_at_the_pointer_at_every_scale() {
    for scale in SCALES {
        let (mut harness, _dir) = opened(support::fixture("anyview-image", "quadrants.png"), scale);
        let pointer = at(300.0, 200.0);
        let rect = right_click(&mut harness, pointer, 0.0).expect("a menu");
        assert!(
            (rect.origin.x.0 - pointer.x.0).abs() <= 2.0
                && (rect.origin.y.0 - pointer.y.0).abs() <= 2.0,
            "{scale}%: corner {:?} at the pointer {pointer:?}",
            rect.origin
        );
    }
}

#[test]
fn a_menu_near_any_edge_or_corner_stays_on_screen_at_every_scale() {
    let spots = [
        (2.0, 70.0),
        (898.0, 70.0),
        (2.0, 598.0),
        (898.0, 598.0),
        (898.0, 300.0),
        (2.0, 300.0),
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = text(dir.path());
    for scale in SCALES {
        for (x, y) in spots {
            let (mut harness, _keep) = opened(path.clone(), scale);
            let rect = right_click(&mut harness, at(x, y), 0.0);
            let rect = rect.unwrap_or_else(|| panic!("{scale}%: no menu at ({x},{y})"));
            assert!(
                on_screen(rect),
                "{scale}%: the menu {rect:?} from ({x},{y}) leaves the {WIDTH}x{HEIGHT} window"
            );
        }
    }
}

#[test]
fn the_menu_key_opens_a_menu_on_screen_at_every_scale() {
    let dir = tempfile::tempdir().unwrap();
    let path = text(dir.path());
    for scale in SCALES {
        let (mut harness, _keep) = opened(path.clone(), scale);
        harness.send(Input::key(ShortcutKey::ContextMenu));
        settle(&mut harness);
        let rect = harness
            .rect(".ds-menu")
            .unwrap_or_else(|| panic!("{scale}%: no menu"));
        assert!(
            on_screen(rect),
            "{scale}%: the menu {rect:?} leaves the window"
        );
    }
}

#[test]
fn a_right_click_over_the_titlebar_opens_no_viewer_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut harness, _keep) = opened(text(dir.path()), 200);
    harness.send(Input::pointer_move(at(450.0, 300.0)));
    settle(&mut harness);
    let bar = harness.centre(".viewer-titlebar").expect("the titlebar");
    assert!(
        right_click(&mut harness, bar, 0.0).is_none(),
        "the titlebar's right-click is its own"
    );
}
