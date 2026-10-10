//! What the window does over time, under the harness on the virtual clock: the arrow keys walk the
//! folder and restore where each file was left, neighbours are opened ahead, a first frame shows
//! before the full open, and a result for a file the person left is dropped.

use crate::support;

use anyview_core::{LineIndex, PixelLen, PixelSize, Resume};
use anyview_image::Rgba8;
use anyview_ui::{HostRequest, WorkKind, WorkLane};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Input, Query};
use std::path::PathBuf;
use std::sync::Arc;
use support::{
    GREEN, Gate, MAGENTA, Memory, Pictures, RED, Wiring, first_line, image_file, is, press, rgb,
    settle, shot, text_file, title, wired,
};

fn gate_that_runs_everything() -> Arc<Gate> {
    Gate::holding(&[])
}

fn magenta_thumbnail() -> Rgba8 {
    let size = PixelSize {
        width: PixelLen(2),
        height: PixelLen(2),
    };
    Rgba8::new(size, [255, 0, 255, 255].repeat(4)).unwrap()
}

#[test]
fn the_arrow_keys_walk_the_folder_and_each_file_opens_where_it_was_left() {
    let dir = tempfile::tempdir().unwrap();
    let a = text_file(dir.path(), "a.txt", "alpha", 400);
    let b = text_file(dir.path(), "b.txt", "bravo", 400);
    let c = text_file(dir.path(), "c.txt", "charlie", 400);
    // The store remembers b from an earlier visit; a is left somewhere new in this one.
    let memory = Memory::with(
        &b,
        Resume::Text {
            line: LineIndex(40),
        },
    );
    let gate = gate_that_runs_everything();
    let wiring = Wiring {
        workers: Some(gate.clone()),
        memory: Some(memory.clone()),
        ..Wiring::default()
    };
    let (mut harness, requests, _) = wired(&[a.clone(), b, c], 0, Appearance::default(), wiring);
    settle(&mut harness);
    assert_eq!(title(&harness).as_deref(), Some("a.txt"));
    assert_eq!(
        first_line(&harness),
        Some(1),
        "a is new, so it opens at its start"
    );

    press(&mut harness, ShortcutKey::PageDown);
    let left_at = first_line(&harness).unwrap();
    assert!(left_at > 1, "a page down scrolls a: {left_at}");

    requests.lock().unwrap().clear();
    // Next: b opens at the line the store remembers, with no probe and no open of its own.
    gate.take_log();
    press(&mut harness, ShortcutKey::Right);
    assert_eq!(title(&harness).as_deref(), Some("b.txt"));
    assert_eq!(first_line(&harness), Some(41), "line 40 is the 41st");
    let kinds: Vec<WorkKind> = gate.take_log().iter().map(|(kind, _)| *kind).collect();
    assert!(
        !kinds.contains(&WorkKind::Probe) && !kinds.contains(&WorkKind::Open),
        "b was opened ahead: {kinds:?}"
    );
    assert!(
        kinds.contains(&WorkKind::Stat),
        "and checked against the disk: {kinds:?}"
    );

    // Previous: a is where the person left it, not where the store last saw it.
    press(&mut harness, ShortcutKey::Left);
    assert_eq!(title(&harness).as_deref(), Some("a.txt"));
    assert_eq!(first_line(&harness), Some(left_at));
    assert_eq!(
        memory.left_at(&a),
        Some(Resume::Text {
            line: LineIndex(left_at - 1)
        }),
        "and the host was told"
    );
}

#[test]
fn the_neighbours_of_the_open_file_are_opened_ahead_on_the_quiet_lane() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = ["a", "b", "c", "d"]
        .iter()
        .map(|name| text_file(dir.path(), &format!("{name}.txt"), name, 20))
        .collect();
    let gate = gate_that_runs_everything();
    let wiring = Wiring {
        workers: Some(gate.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&paths, 1, Appearance::default(), wiring);
    settle(&mut harness);
    let log = gate.take_log();
    let mut ahead = log.iter().filter(|(kind, _)| *kind == WorkKind::Preload);
    assert_eq!(
        ahead.clone().count(),
        2,
        "the file on each side of b: {log:?}"
    );
    assert!(
        ahead.all(|(_, lane)| *lane == WorkLane::Preload),
        "and only they are on the quiet lane: {log:?}"
    );
    assert!(
        log.iter()
            .filter(|(kind, _)| *kind != WorkKind::Preload)
            .all(|(_, lane)| *lane == WorkLane::Visible),
        "what the person waits for is not: {log:?}"
    );
    // Moving on opens the file on the far side of c ahead, and lets a go.
    gate.take_log();
    press(&mut harness, ShortcutKey::Right);
    let log = gate.take_log();
    let preloads = log
        .iter()
        .filter(|(kind, _)| *kind == WorkKind::Preload)
        .count();
    assert_eq!(preloads, 1, "only d is new: {log:?}");
}

#[test]
fn a_first_frame_shows_before_the_full_stage_and_the_full_open_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = image_file(dir.path(), "quadrants.png", "quadrants.png");
    let gate = Gate::holding(&[WorkKind::Open]);
    let wiring = Wiring {
        workers: Some(gate.clone()),
        pictures: Some(Arc::new(Pictures(vec![(
            "quadrants.png".to_owned(),
            magenta_thumbnail(),
        )]))),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    assert_eq!(
        gate.held(WorkKind::Open),
        1,
        "the full open has not come back"
    );
    assert_eq!(
        harness.count(".viewer-raster-picture"),
        1,
        "yet a picture shows"
    );
    let first = harness.render().unwrap();
    if let Some(path) = shot("first-frame.png") {
        first.save(path).unwrap();
    }
    assert!(
        is(rgb(&first, -12.0, -8.0), MAGENTA) && is(rgb(&first, 12.0, 8.0), MAGENTA),
        "the host's small picture fills the box the real one will"
    );
    gate.release(WorkKind::Open);
    settle(&mut harness);
    let full = harness.render().unwrap();
    assert!(
        is(rgb(&full, -12.0, -8.0), RED) && is(rgb(&full, 12.0, -8.0), GREEN),
        "the full open took its place"
    );
}

#[test]
fn a_result_for_a_file_the_person_has_left_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let picture = image_file(dir.path(), "a.png", "quadrants.png");
    let text = text_file(dir.path(), "b.txt", "bravo", 30);
    let gate = Gate::holding(&[WorkKind::Open, WorkKind::Preload]);
    let wiring = Wiring {
        workers: Some(gate.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&[picture, text], 0, Appearance::default(), wiring);
    settle(&mut harness);
    assert_eq!(gate.held(WorkKind::Open), 1, "a's open is out");
    press(&mut harness, ShortcutKey::Right);
    assert_eq!(title(&harness).as_deref(), Some("b.txt"));
    assert_eq!(gate.held(WorkKind::Open), 2, "and b's is too");
    // a's open finishes late, after the person went to b: nothing of it shows.
    gate.release_one(WorkKind::Open);
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-raster"),
        0,
        "a's picture was dropped"
    );
    assert_eq!(title(&harness).as_deref(), Some("b.txt"));
    gate.release_one(WorkKind::Open);
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-raster"), 0);
    assert_eq!(first_line(&harness), Some(1), "b's text is what shows");
}

#[test]
fn the_start_of_a_large_text_shows_before_the_whole_file_is_indexed() {
    let dir = tempfile::tempdir().unwrap();
    // About 700 KB: more than a first frame reads, so there is one.
    let path = text_file(dir.path(), "big.txt", "row", 40_000);
    let gate = Gate::holding(&[WorkKind::Open]);
    let wiring = Wiring {
        workers: Some(gate.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        wiring,
    );
    settle(&mut harness);
    assert_eq!(gate.held(WorkKind::Open), 1, "the open is still out");
    assert_eq!(
        first_line(&harness),
        Some(1),
        "yet the start of the file shows"
    );
    assert!(
        harness
            .text_of(".viewer-code")
            .unwrap()
            .starts_with("row 1")
    );
    gate.release(WorkKind::Open);
    settle(&mut harness);
    assert_eq!(
        first_line(&harness),
        Some(1),
        "and goes on showing as the open lands"
    );
    press(&mut harness, ShortcutKey::End);
    assert!(
        harness
            .text_of(".viewer-text")
            .unwrap()
            .contains("row 40000"),
        "the whole file is reachable once it is open"
    );
}

#[test]
fn a_file_that_is_remembered_far_in_opens_there_when_the_whole_file_is_in() {
    let dir = tempfile::tempdir().unwrap();
    let path = text_file(dir.path(), "big.txt", "row", 40_000);
    let memory = Memory::with(
        &path,
        Resume::Text {
            line: LineIndex(30_000),
        },
    );
    let wiring = Wiring {
        memory: Some(memory),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        wiring,
    );
    settle(&mut harness);
    settle(&mut harness);
    assert_eq!(first_line(&harness), Some(30_001));
}

#[test]
fn closing_the_window_keeps_where_the_person_was() {
    let dir = tempfile::tempdir().unwrap();
    let path = text_file(dir.path(), "a.txt", "alpha", 400);
    let (mut harness, requests, _) = wired(
        std::slice::from_ref(&path),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    press(&mut harness, ShortcutKey::PageDown);
    let line = first_line(&harness).unwrap() - 1;
    requests.lock().unwrap().clear();
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('w')));
    settle(&mut harness);
    let asked = requests.lock().unwrap();
    assert_eq!(
        asked.as_slice(),
        [
            HostRequest::Remember(Resume::Text {
                line: LineIndex(line)
            }),
            HostRequest::Unwatch,
            HostRequest::CloseWindow,
        ],
        "the place, then the end of the watch, then the window"
    );
}
