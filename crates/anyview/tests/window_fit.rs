//! A window sized to its first file once it has loaded: a PDF opened into a new window asks for its
//! page's size, once; a window the person has resized is left alone; and moving on with the arrow
//! keys never resizes. The window's sizer is a stand-in that records what it was asked, since a
//! harness window has no real window to size.

#![allow(clippy::unwrap_used)]

#[path = "../../anyview-pdf/tests/support/mod.rs"]
mod pdf_fixture;
mod support;

use anyview::media::MediaPlugins;
use anyview::window::{Sizer, SizerContext};
use ds::prelude::ShortcutKey;
use ds_blitz::{Extent, SizeOrigin};
use ds_harness::{Driver, Input, Query};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use support::{Rig, fixture, open_sized, until};

/// What the window was asked, and who last resized it.
#[derive(Debug)]
struct Asked {
    origin: Option<SizeOrigin>,
    size: Extent,
    requests: Vec<Extent>,
}

struct StandIn(Arc<Mutex<Asked>>);

impl Sizer for StandIn {
    fn origin(&self) -> Option<SizeOrigin> {
        self.0.lock().unwrap().origin
    }

    fn size(&self) -> Extent {
        self.0.lock().unwrap().size
    }

    fn request(&self, size: Extent) {
        let mut asked = self.0.lock().unwrap();
        asked.requests.push(size);
        asked.size = size;
        asked.origin = Some(SizeOrigin::Requested);
    }
}

/// A window that opened at the default size, with nobody having resized it yet.
fn untouched() -> Arc<Mutex<Asked>> {
    Arc::new(Mutex::new(Asked {
        origin: None,
        size: Extent::new(1000, 700),
        requests: Vec::new(),
    }))
}

fn sizer_of(asked: &Arc<Mutex<Asked>>) -> SizerContext {
    let asked = Arc::clone(asked);
    SizerContext::new(move || Rc::new(StandIn(Arc::clone(&asked))) as Rc<dyn Sizer>)
}

/// A folder of `a.pdf` (its first page 612 by 792) and `b.png` (a 900 by 500 picture), opened on
/// the PDF.
fn pdf_then_picture(dir: &Path, asked: &Arc<Mutex<Asked>>) -> Rig {
    let pdf = dir.join("a.pdf");
    std::fs::write(&pdf, pdf_fixture::fixture_bytes()).unwrap();
    image::GrayImage::new(900, 500)
        .save(dir.join("b.png"))
        .unwrap();
    open_sized(
        &pdf,
        dir,
        Arc::new(MediaPlugins::default()),
        Some(sizer_of(asked)),
    )
}

fn requests(asked: &Arc<Mutex<Asked>>) -> Vec<Extent> {
    asked.lock().unwrap().requests.clone()
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

#[test]
fn a_pdf_opened_into_a_new_window_asks_for_its_page_size_once() {
    let dir = tempfile::tempdir().unwrap();
    let asked = untouched();
    let mut rig = pdf_then_picture(dir.path(), &asked);
    until(&mut rig.harness, "the PDF's page", |harness| {
        harness.count(".viewer-pdf-page") > 0
    });
    until(&mut rig.harness, "the window's size asked for", |_| {
        !requests(&asked).is_empty()
    });
    settle(&mut rig);
    assert_eq!(
        requests(&asked),
        vec![Extent::new(612, 792)],
        "the first page at 100%, once"
    );
}

#[test]
fn a_window_the_person_resized_is_never_resized_again() {
    let dir = tempfile::tempdir().unwrap();
    let asked = untouched();
    asked.lock().unwrap().origin = Some(SizeOrigin::Person);
    let mut rig = pdf_then_picture(dir.path(), &asked);
    until(&mut rig.harness, "the PDF's page", |harness| {
        harness.count(".viewer-pdf-page") > 0
    });
    settle(&mut rig);
    assert_eq!(requests(&asked), vec![], "the person's size stands");
}

#[test]
fn the_arrow_keys_move_to_the_next_file_and_never_resize() {
    let dir = tempfile::tempdir().unwrap();
    let asked = untouched();
    let mut rig = pdf_then_picture(dir.path(), &asked);
    until(&mut rig.harness, "the window's size asked for", |_| {
        !requests(&asked).is_empty()
    });
    settle(&mut rig);
    rig.harness.send(Input::key(ShortcutKey::Right));
    until(&mut rig.harness, "the picture", |harness| {
        harness.count(".viewer-raster") > 0
    });
    assert_eq!(title(&rig).as_deref(), Some("b.png"));
    settle(&mut rig);
    rig.harness.send(Input::key(ShortcutKey::Left));
    until(&mut rig.harness, "the PDF again", |harness| {
        harness.count(".viewer-pdf-page") > 0
    });
    settle(&mut rig);
    assert_eq!(
        requests(&asked),
        vec![Extent::new(612, 792)],
        "only the window's first file sized it"
    );
}

#[test]
fn a_picture_whose_header_gave_its_size_asks_for_nothing_after_load() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("quadrants.png");
    std::fs::copy(fixture("anyview-image", "quadrants.png"), &file).unwrap();
    let asked = untouched();
    // The window opened at the size the header gave (the least, for a small picture).
    asked.lock().unwrap().size = Extent::new(480, 320);
    let mut rig = open_sized(
        &file,
        dir.path(),
        Arc::new(MediaPlugins::default()),
        Some(sizer_of(&asked)),
    );
    until(&mut rig.harness, "the picture", |harness| {
        harness.count(".viewer-raster") > 0
    });
    settle(&mut rig);
    assert_eq!(requests(&asked), vec![]);
}
