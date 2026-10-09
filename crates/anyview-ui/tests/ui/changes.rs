//! A file that changes on disk reloads where the person was, and a file dropped on the window
//! opens with its folder as the list the arrows walk. Under the harness on the virtual clock.

use crate::support;

use anyview_core::FilePath;
use anyview_ui::{HostRequest, WorkKind};
use ds::file_drop::drag::{FileDragInput, Offer};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Input, Query};
use image::RgbaImage;
use std::sync::Arc;
use std::time::Duration;
use support::{
    GREEN, Gate, Wiring, first_line, image_file, is, middle, press, rgb, settle, text_file, title,
    wired,
};

fn gate_that_runs_everything() -> Arc<Gate> {
    Gate::holding(&[])
}

#[test]
fn a_changed_text_file_reloads_at_the_line_the_person_was_on() {
    let dir = tempfile::tempdir().unwrap();
    let path = text_file(dir.path(), "log.txt", "entry", 400);
    let (mut harness, _requests, edge) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    press(&mut harness, ShortcutKey::PageDown);
    press(&mut harness, ShortcutKey::PageDown);
    let line = first_line(&harness).unwrap();
    assert!(line > 20, "scrolled well in: {line}");
    let before = harness.text_of(".viewer-code").unwrap();
    assert!(before.starts_with("entry"), "{before}");

    // The file is edited on disk and the host says so.
    let edited: String = (1..=400).map(|n| format!("revised entry {n}\n")).collect();
    std::fs::write(&path, edited).unwrap();
    edge.changed(FilePath::new(&path).unwrap());
    settle(&mut harness);
    settle(&mut harness);
    assert_eq!(first_line(&harness), Some(line), "the line is kept");
    let after = harness.text_of(".viewer-code").unwrap();
    assert!(after.starts_with("revised"), "the new copy shows: {after}");
}

#[test]
fn a_file_that_did_not_change_is_not_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    let path = text_file(dir.path(), "log.txt", "entry", 50);
    let gate = gate_that_runs_everything();
    let wiring = Wiring {
        workers: Some(gate.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, edge) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        wiring,
    );
    settle(&mut harness);
    gate.take_log();
    edge.changed(FilePath::new(&path).unwrap());
    settle(&mut harness);
    let kinds: Vec<WorkKind> = gate.take_log().iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        vec![WorkKind::Stat],
        "a stamp is read and that is all"
    );
}

#[test]
fn a_changed_picture_reloads_at_the_zoom_the_person_chose() {
    let dir = tempfile::tempdir().unwrap();
    let path = image_file(dir.path(), "p.png", "quadrants.png");
    let (mut harness, _, edge) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    harness.send(Input::pointer_move(middle()));
    harness.advance(Duration::from_millis(300));
    press(&mut harness, ShortcutKey::Char('+'));
    assert_eq!(
        harness.text_of(".ds-capsule-readout").as_deref(),
        Some("125%")
    );
    let green = RgbaImage::from_pixel(64, 48, image::Rgba([0, 255, 0, 255]));
    green.save(&path).unwrap();
    edge.changed(FilePath::new(&path).unwrap());
    settle(&mut harness);
    settle(&mut harness);
    assert_eq!(
        harness.text_of(".ds-capsule-readout").as_deref(),
        Some("125%"),
        "the zoom is kept"
    );
    let image = harness.render().unwrap();
    assert!(is(rgb(&image, 0.0, 0.0), GREEN), "the new picture shows");
}

#[test]
fn a_dropped_file_opens_and_its_folder_becomes_the_list_the_arrows_walk() {
    let here = tempfile::tempdir().unwrap();
    let there = tempfile::tempdir().unwrap();
    let start = text_file(here.path(), "start.txt", "start", 10);
    let first = text_file(there.path(), "x1.txt", "ex", 10);
    text_file(there.path(), "x2.txt", "why", 10);
    let (mut harness, requests, _) = wired(
        std::slice::from_ref(&start),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    assert_eq!(title(&harness).as_deref(), Some("start.txt"));
    for step in [
        FileDragInput::Entered {
            point: Some(middle()),
        },
        FileDragInput::Offered(Offer::Files(vec![first.clone()])),
        FileDragInput::Moved { point: middle() },
        FileDragInput::Dropped,
    ] {
        harness.send(Input::FileDrag(step));
    }
    settle(&mut harness);
    settle(&mut harness);
    assert_eq!(
        title(&harness).as_deref(),
        Some("x1.txt"),
        "the dropped file opens"
    );
    press(&mut harness, ShortcutKey::Right);
    assert_eq!(
        title(&harness).as_deref(),
        Some("x2.txt"),
        "and its folder is what the arrows walk"
    );
    let asked = requests.lock().unwrap();
    assert!(
        asked.iter().any(|request| matches!(
            request,
            HostRequest::Opened(probed) if probed.source.path().as_path() == first
        )),
        "the host was told: {asked:?}"
    );
}
