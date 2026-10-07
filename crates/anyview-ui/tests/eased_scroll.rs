//! A wheel's detent in the picture and the PDF views: the window's scroll step eases it over
//! frames on the virtual clock, and the frames sum to the 60 px of a detent (design/11 §11.3.11),
//! where they used to jump the whole detent at once. A touchpad's run (`Began`, `Changed`, then the
//! glide after a fast lift, then `Ended`) moves the content with the fingers, carries on a little
//! and stops.

#![allow(clippy::unwrap_used)]

#[path = "../../anyview-pdf/tests/support/mod.rs"]
mod pdf_fixture;
mod support;

use ds::host::gesture::GesturePhase;
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::time::Duration;
use support::{VIEW, folder, window};

/// One frame, as the window's 120 Hz at its slowest.
const FRAME: Duration = Duration::from_millis(8);

fn middle() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

/// `read` after each of `frames` frames, starting with the one now.
fn trace(harness: &mut Harness, frames: usize, read: impl Fn(&Harness) -> f32) -> Vec<f32> {
    let mut seen = vec![read(harness)];
    for _ in 0..frames {
        harness.advance(FRAME);
        seen.push(read(harness));
    }
    seen
}

/// What a detent's trace must show: nothing before the first frame, intermediate frames and not
/// a jump, one direction only, all within 200 ms, and 60 px (within a pixel) in the end.
fn assert_eased_detent(seen: &[f32]) {
    let start = seen[0];
    let moved: Vec<f32> = seen.iter().map(|at| (at - start).abs()).collect();
    assert!(moved[0] < 1e-3, "nothing moves before a frame: {moved:?}");
    let between = moved.iter().filter(|m| **m > 1.0 && **m < 59.0).count();
    assert!(between >= 3, "intermediate frames, not a jump: {moved:?}");
    assert!(
        moved.windows(2).all(|pair| pair[1] + 0.51 >= pair[0]),
        "it only moves one way: {moved:?}"
    );
    let end = *moved.last().unwrap();
    assert!((end - 60.0).abs() <= 1.0, "a detent is 60 px: {end}");
    let arrived = moved.iter().position(|m| (m - 60.0).abs() <= 1.0).unwrap();
    assert!(
        FRAME * u32::try_from(arrived).unwrap() <= Duration::from_millis(208),
        "within 200 ms: frame {arrived}"
    );
}

#[test]
fn a_wheel_detent_eases_the_pdf_over_frames_and_sums_to_60_px() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.pdf");
    std::fs::write(&path, pdf_fixture::fixture_bytes()).unwrap();
    let paths = vec![std::fs::canonicalize(path).unwrap()];
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(500));
    let top = |harness: &Harness| harness.rect(".viewer-pdf-page").unwrap().origin.y.0;
    // winit's sign: a click that scrolls the page down is -1.
    harness.send(Input::detents(middle(), 0.0, -1.0));
    let seen = trace(&mut harness, 30, top);
    assert_eased_detent(&seen);
    assert!(seen.last().unwrap() < &seen[0], "the page moved up");
}

#[test]
fn a_wheel_detent_eases_a_zoomed_picture_over_frames_and_sums_to_60_px() {
    let (_dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(300));
    // Zoomed well past the window, so there is room to pan.
    for _ in 0..12 {
        harness.send(Input::key(ShortcutKey::Char('+')));
    }
    harness.advance(Duration::from_millis(300));
    let left = |harness: &Harness| harness.rect(".viewer-raster-picture").unwrap().origin.y.0;
    harness.send(Input::detents(middle(), 0.0, -1.0));
    let seen = trace(&mut harness, 30, left);
    assert_eased_detent(&seen);
}

/// Fingers moving `step` px every frame for `moves` frames and lifting, then the glide to its end:
/// what `read` showed before the first move, at the lift, and once the glide had stopped.
fn flick(harness: &mut Harness, step: f32, moves: u32, read: impl Fn(&Harness) -> f32) -> [f32; 3] {
    let before = read(harness);
    for n in 0..moves {
        let phase = if n == 0 {
            GesturePhase::Began
        } else {
            GesturePhase::Changed
        };
        harness.send(Input::fingers(middle(), Px(0.0), Px(step), phase));
        harness.advance(FRAME);
    }
    harness.send(Input::fingers(
        middle(),
        Px(0.0),
        Px(0.0),
        GesturePhase::Ended,
    ));
    let at_lift = read(harness);
    // The glide is over well within three seconds; a frame later nothing moves.
    for _ in 0..375 {
        harness.advance(FRAME);
    }
    let end = read(harness);
    harness.advance(FRAME);
    assert!(
        (read(harness) - end).abs() < 1e-3,
        "the glide has stopped, not crept on"
    );
    [before, at_lift, end]
}

#[test]
fn a_touchpad_run_moves_the_pdf_with_the_fingers_glides_on_and_stops() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.pdf");
    std::fs::write(&path, pdf_fixture::fixture_bytes()).unwrap();
    let paths = vec![std::fs::canonicalize(path).unwrap()];
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(500));
    let top = |harness: &Harness| harness.rect(".viewer-pdf-page").unwrap().origin.y.0;
    // The content follows the fingers: fingers moving up (winit's negative) carry the page up.
    // Small enough that the glide stays on the first page, whose own top is what is read.
    let [before, at_lift, end] = flick(&mut harness, -6.0, 6, top);
    assert!(
        at_lift < before - 30.0,
        "it follows the fingers: {before} to {at_lift}"
    );
    assert!(
        end < at_lift - 5.0,
        "it glides on after a fast lift: {at_lift} to {end}"
    );
}

#[test]
fn a_touchpad_run_moves_a_zoomed_picture_with_the_fingers_glides_on_and_stops() {
    let (_dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(300));
    for _ in 0..12 {
        harness.send(Input::key(ShortcutKey::Char('+')));
    }
    harness.advance(Duration::from_millis(300));
    let top = |harness: &Harness| harness.rect(".viewer-raster-picture").unwrap().origin.y.0;
    let [before, at_lift, end] = flick(&mut harness, -20.0, 6, top);
    assert!(
        at_lift < before - 60.0,
        "it follows the fingers: {before} to {at_lift}"
    );
    assert!(end <= at_lift, "it never comes back: {at_lift} to {end}");
}
