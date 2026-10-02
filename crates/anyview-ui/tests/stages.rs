//! The stages on screen under the harness: a picture is uploaded to the GPU and drawn where the
//! machine's state says, a pinch and a key zoom it, a turn rotates it, a wheel scrolls a window of
//! lines, Markdown is a sealed frame, and a file with no stage offers Open With….

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FileAction;
use anyview_ui::HostRequest;
use ds::host::gesture::{Gesture, GesturePhase, Magnification};
use ds::prelude::{Appearance, Point, Px, ShortcutKey, Theme};
use ds_harness::{Driver, Harness, Input, Query};
use image::RgbaImage;
use std::time::Duration;
use support::{VIEW, folder, shot, window};

fn centre() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

fn rgb(image: &RgbaImage, dx: f32, dy: f32) -> [u8; 3] {
    let at = (
        (centre().x.0 + dx).round() as u32,
        (centre().y.0 + dy).round() as u32,
    );
    let pixel = image.get_pixel(at.0, at.1);
    [pixel[0], pixel[1], pixel[2]]
}

fn is(colour: [u8; 3], want: [u8; 3]) -> bool {
    colour
        .iter()
        .zip(want)
        .all(|(got, want)| (i32::from(*got) - i32::from(want)).abs() < 40)
}

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const YELLOW: [u8; 3] = [255, 255, 0];

fn picture() -> (tempfile::TempDir, Harness) {
    let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(300));
    (dir, harness)
}

fn save(harness: &mut Harness, name: &str) {
    if let Some(path) = shot(name) {
        harness.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn a_picture_is_decoded_uploaded_and_drawn_at_its_own_size_in_the_middle() {
    let (_dir, mut harness) = picture();
    let image = harness.render().unwrap();
    // The 48 x 32 picture is four quadrants of 24 x 16, centred on the window.
    for (name, dx, dy, want) in [
        ("top left", -12.0, -8.0, RED),
        ("top right", 12.0, -8.0, GREEN),
        ("bottom left", -12.0, 8.0, BLUE),
        ("bottom right", 12.0, 8.0, YELLOW),
    ] {
        let got = rgb(&image, dx, dy);
        assert!(is(got, want), "{name}: {got:?} is not {want:?}");
    }
    assert_eq!(
        harness.attr(".viewer-raster", "data-zoom").as_deref(),
        Some("fit")
    );
    save(&mut harness, "picture.png");
}

#[test]
fn a_key_and_a_pinch_zoom_the_picture_and_the_capsule_says_how_far() {
    let (_dir, mut harness) = picture();
    harness.send(Input::pointer_move(centre()));
    harness.advance(Duration::from_millis(300));
    let readout = |harness: &Harness| harness.text_of(".ds-capsule-readout");
    assert_eq!(readout(&harness).as_deref(), Some("100%"));
    harness.send(Input::key(ShortcutKey::Char('+')));
    harness.advance(Duration::from_millis(100));
    assert_eq!(
        readout(&harness).as_deref(),
        Some("125%"),
        "one step is x1.25"
    );
    assert_eq!(
        harness.attr(".viewer-raster", "data-zoom").as_deref(),
        Some("zoomed")
    );
    harness.send(Input::gesture(Gesture::Pinch {
        phase: GesturePhase::Changed,
        by: Magnification(200),
        at: centre(),
    }));
    harness.advance(Duration::from_millis(100));
    assert_eq!(
        readout(&harness).as_deref(),
        Some("150%"),
        "a pinch of 20% from 125%"
    );
    harness.send(Input::key(ShortcutKey::Char('0')));
    harness.advance(Duration::from_millis(100));
    assert_eq!(readout(&harness).as_deref(), Some("100%"), "0 fits again");
    assert_eq!(
        harness.attr(".viewer-raster", "data-zoom").as_deref(),
        Some("fit")
    );
    // The picture is bigger when zoomed: the red quadrant now reaches past its 24 px.
    harness.send(Input::key(ShortcutKey::Char('1')));
    harness.advance(Duration::from_millis(100));
    save(&mut harness, "picture-actual.png");
}

#[test]
fn rotating_right_turns_each_quadrant_a_quarter_clockwise() {
    let (_dir, mut harness) = picture();
    harness.send(Input::pointer_move(centre()));
    harness.advance(Duration::from_millis(300));
    let button = harness
        .centre("[aria-label=\"Rotate right\"]")
        .expect("the rotate button");
    harness.send(Input::click(button));
    harness.advance(Duration::from_millis(100));
    let image = harness.render().unwrap();
    // Turned a quarter clockwise the picture is 32 wide and 48 tall: red moves to the top right.
    for (name, dx, dy, want) in [
        ("top left", -8.0, -12.0, BLUE),
        ("top right", 8.0, -12.0, RED),
        ("bottom left", -8.0, 12.0, YELLOW),
        ("bottom right", 8.0, 12.0, GREEN),
    ] {
        let got = rgb(&image, dx, dy);
        assert!(is(got, want), "{name}: {got:?} is not {want:?}");
    }
    save(&mut harness, "picture-rotated.png");
}

#[test]
fn a_wheel_scrolls_a_text_file_by_whole_lines_and_the_lines_are_highlighted() {
    let (_dir, paths) = folder(&[("anyview-text", "sample.rs", "sample.rs")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(300));
    let first = |harness: &Harness| harness.text_of(".viewer-lineno");
    assert_eq!(first(&harness).as_deref(), Some("1"));
    assert!(
        harness.count(".tok-keyword") > 0,
        "keywords are classed for the stylesheet"
    );
    harness.send(Input::wheel(centre(), Px(0.0), Px(-54.0)));
    harness.advance(Duration::from_millis(300));
    assert_eq!(
        first(&harness).as_deref(),
        Some("4"),
        "three lines of 18 px"
    );
    harness.send(Input::wheel(centre(), Px(0.0), Px(18.0)));
    harness.advance(Duration::from_millis(300));
    assert_eq!(first(&harness).as_deref(), Some("3"), "one line back");
    save(&mut harness, "code.png");
}

#[test]
fn markdown_is_a_page_in_a_sealed_frame_and_v_shows_its_source() {
    let (_dir, paths) = folder(&[("anyview-text", "readme.md", "readme.md")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(500));
    assert_eq!(
        harness.attr(".viewer-text", "data-view").as_deref(),
        Some("rendered")
    );
    assert_eq!(harness.count("iframe.viewer-frame"), 1);
    let page = harness
        .frame("iframe.viewer-frame")
        .expect("the frame has a document");
    assert!(page.count("h1") > 0, "the page has its heading");
    save(&mut harness, "markdown.png");
    harness.send(Input::key(ShortcutKey::Char('v')));
    harness.advance(Duration::from_millis(300));
    assert_eq!(
        harness.attr(".viewer-text", "data-view").as_deref(),
        Some("source")
    );
    assert_eq!(harness.count("iframe.viewer-frame"), 0);
    assert!(harness.count(".viewer-line") > 0);
}

#[test]
fn a_file_with_no_stage_shows_its_facts_and_hands_over_with_open_with() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mystery.bin");
    std::fs::write(&path, [0u8, 159, 146, 150, 0, 1, 2, 3]).unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let (mut harness, requests) = window(std::slice::from_ref(&path), 0, Appearance::default());
    harness.advance(Duration::from_millis(300));
    assert_eq!(harness.count(".viewer-peek"), 1);
    assert_eq!(
        harness.text_of(".ds-empty-state-title").as_deref(),
        Some("mystery.bin")
    );
    assert!(harness.count(".ds-fact-list") > 0, "its facts are listed");
    let open_with = harness
        .centre(".viewer-peek .ds-button")
        .expect("Open With…");
    harness.send(Input::click(open_with));
    harness.advance(Duration::from_millis(100));
    let requests = requests.lock().unwrap();
    assert!(
        matches!(
            requests.as_slice(),
            [HostRequest::Opened(probed), HostRequest::Run(FileAction::OpenWith)]
                if probed.source.path().as_path() == path
        ),
        "the window told its host which file it shows, then asked to open it with: {requests:?}"
    );
    drop(requests);
    save(&mut harness, "peek-only.png");
}

#[test]
fn the_window_wears_the_dark_look_too() {
    let (_dir, paths) = folder(&[("anyview-text", "sample.rs", "sample.rs")]);
    let dark = Appearance {
        theme: Theme::Dark,
        ..Appearance::default()
    };
    let (mut harness, _) = window(&paths, 0, dark);
    harness.send(Input::pointer_move(centre()));
    harness.advance(Duration::from_millis(500));
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    harness.advance(Duration::from_millis(500));
    save(&mut harness, "palette-dark.png");
    assert!(harness.count(".ds-palette") > 0);
}
