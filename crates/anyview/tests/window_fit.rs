//! A window sized to its first file once it has loaded: a PDF opened into a new window asks for its
//! page's size, once; a picture is asked for at one image pixel to one device pixel; a window the
//! person has resized is left alone; and moving on with the arrow keys never resizes. The window is
//! the harness's, which records what it is asked and answers as the test chose (`SizerAck`).

#![allow(clippy::unwrap_used)]

#[path = "../../anyview-pdf/tests/support/mod.rs"]
mod pdf_fixture;
mod support;

use anyview::media::MediaPlugins;
use ds::prelude::{Scale, ShortcutKey};
use ds_blitz::{Extent, ScreenArea, ScreenOf};
use ds_harness::{Driver, Input, Query, SizerAck, WindowScreen};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use support::{Rig, VIEW, Wired, open_wired, until};

/// A folder of `a.pdf` (its first page 612 by 792) and `b.png` (a 900 by 500 picture), opened on
/// the PDF.
fn pdf_then_picture(dir: &Path, wired: Wired) -> Rig {
    let pdf = dir.join("a.pdf");
    std::fs::write(&pdf, pdf_fixture::fixture_bytes()).unwrap();
    image::GrayImage::new(900, 500)
        .save(dir.join("b.png"))
        .unwrap();
    open_wired(&pdf, dir, Arc::new(MediaPlugins::default()), wired)
}

/// `pixels` of a picture on its own in a folder, opened in a window of the harness's size.
fn picture(dir: &Path, pixels: Extent, wired: Wired) -> Rig {
    let file = dir.join("picture.png");
    image::GrayImage::new(pixels.width, pixels.height)
        .save(&file)
        .unwrap();
    open_wired(&file, dir, Arc::new(MediaPlugins::default()), wired)
}

fn requests(rig: &Rig) -> Vec<Extent> {
    rig.harness.window_requests()
}

fn title(rig: &Rig) -> Option<String> {
    rig.harness.text_of(".ds-titlebar-name")
}

/// Let the window run on a while, so a request that is going to come has come.
fn settle(rig: &mut Rig) {
    for _ in 0..40 {
        rig.harness.advance(Duration::from_millis(25));
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn page_is_up(rig: &mut Rig) {
    until(&mut rig.harness, "the PDF's page", |harness| {
        harness.count(".viewer-pdf-page") > 0
    });
}

fn picture_is_up(rig: &mut Rig) {
    until(&mut rig.harness, "the picture", |harness| {
        harness.count(".viewer-raster") > 0
    });
}

#[test]
fn a_pdf_opened_into_a_new_window_asks_for_its_page_size_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut rig = pdf_then_picture(dir.path(), Wired::default());
    page_is_up(&mut rig);
    until(&mut rig.harness, "the window's size asked for", |harness| {
        !harness.window_requests().is_empty()
    });
    settle(&mut rig);
    assert_eq!(
        requests(&rig),
        vec![Extent::new(612, 792)],
        "the first page at 100%, once"
    );
    assert_eq!(
        rig.harness.window_size(),
        Extent::new(612, 792),
        "and the window took it"
    );
}

#[test]
fn a_page_is_the_same_logical_size_on_a_hidpi_screen() {
    let dir = tempfile::tempdir().unwrap();
    let wired = Wired {
        screen: WindowScreen::Area(
            ScreenArea::new(Extent::new(3840, 2160), Scale(240), ScreenOf::Window).unwrap(),
        ),
        ..Wired::default()
    };
    let mut rig = pdf_then_picture(dir.path(), wired);
    page_is_up(&mut rig);
    settle(&mut rig);
    assert_eq!(
        requests(&rig),
        vec![Extent::new(612, 792)],
        "a point is a logical pixel"
    );
}

#[test]
fn a_picture_is_asked_for_at_one_image_pixel_to_one_device_pixel() {
    let cases = [
        // name, physical output, scale, picture pixels, the window asked for
        ("1x", (1920, 1080), 120, (1200, 800), (1200, 800)),
        ("2x", (3840, 2160), 240, (1200, 800), (600, 400)),
        ("1.5x", (3840, 2160), 180, (1200, 800), (800, 533)),
        (
            "2x, larger than the screen",
            (3840, 2160),
            240,
            (8000, 4000),
            (1632, 816),
        ),
    ];
    for (name, output, scale, pixels, want) in cases {
        let dir = tempfile::tempdir().unwrap();
        let screen = ScreenArea::new(
            Extent::new(output.0, output.1),
            Scale(scale),
            ScreenOf::Window,
        )
        .unwrap();
        let wired = Wired {
            screen: WindowScreen::Area(screen),
            ..Wired::default()
        };
        let mut rig = picture(dir.path(), Extent::new(pixels.0, pixels.1), wired);
        picture_is_up(&mut rig);
        settle(&mut rig);
        assert_eq!(requests(&rig), vec![Extent::new(want.0, want.1)], "{name}");
    }
}

#[test]
fn a_window_the_person_resized_after_it_was_sized_stays_as_they_left_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut rig = pdf_then_picture(dir.path(), Wired::default());
    page_is_up(&mut rig);
    settle(&mut rig);
    rig.harness.resize_window(Extent::new(800, 500));
    rig.harness.send(Input::key(ShortcutKey::Right));
    picture_is_up(&mut rig);
    settle(&mut rig);
    assert_eq!(requests(&rig), vec![Extent::new(612, 792)], "asked once");
    assert_eq!(rig.harness.window_size(), Extent::new(800, 500));
}

#[test]
fn the_arrow_keys_move_to_the_next_file_and_never_resize() {
    let dir = tempfile::tempdir().unwrap();
    let mut rig = pdf_then_picture(dir.path(), Wired::default());
    until(&mut rig.harness, "the window's size asked for", |harness| {
        !harness.window_requests().is_empty()
    });
    settle(&mut rig);
    rig.harness.send(Input::key(ShortcutKey::Right));
    picture_is_up(&mut rig);
    assert_eq!(title(&rig).as_deref(), Some("b.png"));
    settle(&mut rig);
    rig.harness.send(Input::key(ShortcutKey::Left));
    page_is_up(&mut rig);
    settle(&mut rig);
    assert_eq!(
        requests(&rig),
        vec![Extent::new(612, 792)],
        "only the window's first file sized it"
    );
}

#[test]
fn a_picture_whose_size_the_window_already_is_asks_for_nothing_after_load() {
    let dir = tempfile::tempdir().unwrap();
    // The harness's window is VIEW, as a window the header sized is its picture's.
    let mut rig = picture(
        dir.path(),
        Extent::new(VIEW.width, VIEW.height),
        Wired::default(),
    );
    picture_is_up(&mut rig);
    settle(&mut rig);
    assert_eq!(requests(&rig), vec![]);
}

#[test]
fn a_compositor_that_ignores_the_request_is_not_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let wired = Wired {
        ack: SizerAck::Never,
        ..Wired::default()
    };
    let mut rig = pdf_then_picture(dir.path(), wired);
    page_is_up(&mut rig);
    settle(&mut rig);
    // Past quire's 500 ms for an answer, and the request is spent, not retried.
    rig.harness.advance(Duration::from_millis(600));
    settle(&mut rig);
    assert_eq!(requests(&rig), vec![Extent::new(612, 792)]);
    assert_eq!(
        rig.harness.window_size(),
        Extent::new(VIEW.width, VIEW.height)
    );
}

#[test]
fn a_slow_answer_is_the_windows_own_resize_and_the_window_is_not_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let wired = Wired {
        ack: SizerAck::After(Duration::from_millis(200)),
        ..Wired::default()
    };
    let mut rig = pdf_then_picture(dir.path(), wired);
    page_is_up(&mut rig);
    settle(&mut rig);
    assert_eq!(requests(&rig), vec![Extent::new(612, 792)]);
    assert_eq!(rig.harness.window_size(), Extent::new(612, 792));
}
