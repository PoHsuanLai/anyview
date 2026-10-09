//! The right-click menu where a person really uses it: over the picture itself, at the scales a
//! desktop runs at, with a hand that moves a little, near the window's edges.

use crate::support;

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

/// Where a row's right-click lands.
#[derive(Clone, Copy)]
enum How {
    /// The middle of the window, over whatever the content draws there.
    Middle,
    /// The middle of the picture: a texture layer is a custom widget, and Blitz hands a widget its
    /// pointer events and returns before it makes the click and the contextmenu out of a release
    /// (blitz-dom events/mod.rs, "Handle event forwarding for custom widget"), so no menu comes.
    OverThePicture,
}

/// Every kind of file the viewer claims: (row name, fixture folder, fixture, where the click lands).
/// The folder `formats` is this crate's own.
const CASES: &[(&str, &str, &str, How)] = &[
    ("png", "anyview-image", "quadrants.png", How::OverThePicture),
    ("gif", "anyview-image", "spin.gif", How::OverThePicture),
    ("svg", "anyview-image", "logo.svg", How::OverThePicture),
    ("svg without a size", "formats", "nosize.svg", How::Middle),
    ("pdf", "formats", "multi.pdf", How::Middle),
    ("csv", "formats", "ragged.csv", How::Middle),
    ("json", "formats", "a.json", How::Middle),
    // A rendered Markdown page and an EPUB chapter are frames: the right-click has to come out of
    // the frame's document to reach the window's menu.
    ("md", "formats", "r.md", How::Middle),
    ("epub", "formats", "book.epub", How::Middle),
];

#[test]
fn a_right_click_over_each_kind_opens_the_menu() {
    for &(name, home, file, how) in CASES {
        let path = if home == "formats" {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/formats")
                .join(file)
        } else {
            support::fixture(home, file)
        };
        for scale in SCALES {
            let (mut harness, _dir) = opened(path.clone(), scale);
            let at = match how {
                How::Middle => at(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0),
                How::OverThePicture => {
                    let picture = harness.rect(".viewer-raster-picture").unwrap();
                    at(
                        picture.origin.x.0 + picture.size.width.0 / 2.0,
                        picture.origin.y.0 + picture.size.height.0 / 2.0,
                    )
                }
            };
            let menu = right_click(&mut harness, at, 0.0);
            assert!(
                menu.is_some(),
                "row {name} ({file}) at {scale}%: a right-click at {at:?} opens no menu"
            );
        }
    }
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
        let (mut harness, _keep) = opened(path.clone(), scale);
        for (x, y) in spots {
            let rect = right_click(&mut harness, at(x, y), 0.0);
            let rect = rect.unwrap_or_else(|| panic!("{scale}%: no menu at ({x},{y})"));
            assert!(
                on_screen(rect),
                "{scale}%: the menu {rect:?} from ({x},{y}) leaves the {WIDTH}x{HEIGHT} window"
            );
            harness.send(Input::key(ShortcutKey::Escape));
            settle(&mut harness);
            assert!(
                harness.rect(".ds-menu").is_none(),
                "{scale}%: Escape leaves the menu from ({x},{y}) open"
            );
        }
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
