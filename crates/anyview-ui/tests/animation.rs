//! An animated image under the harness on the virtual clock: its frames play at the delays the
//! file asked for, Space pauses it on a frame and plays it again.

#![allow(clippy::unwrap_used)]

mod support;

use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input};
use std::time::Duration;
use support::{VIEW, folder, window};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];

fn centre() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

/// Which of the three frames is in the middle of the window.
fn frame(harness: &mut Harness) -> Option<usize> {
    let image = harness.render().unwrap();
    let pixel = image.get_pixel(centre().x.0 as u32, centre().y.0 as u32);
    [RED, GREEN, BLUE].iter().position(|want| {
        want.iter()
            .zip(pixel.0)
            .all(|(want, got)| (i32::from(*want) - i32::from(got)).abs() < 40)
    })
}

/// The frames seen every `step` for `total`.
fn watch(harness: &mut Harness, step: Duration, total: Duration) -> Vec<Option<usize>> {
    let mut seen = Vec::new();
    let mut elapsed = Duration::ZERO;
    while elapsed < total {
        harness.advance(step);
        elapsed += step;
        seen.push(frame(harness));
    }
    seen
}

#[test]
fn an_animated_gif_plays_its_frames_pauses_on_one_and_plays_again() {
    let (_dir, paths) = folder(&[("anyview-image", "spin.gif", "spin.gif")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(300));
    // The file's delays are 50, 200 and 400 ms: over two turns every frame comes up, in order.
    let playing = watch(
        &mut harness,
        Duration::from_millis(25),
        Duration::from_millis(1400),
    );
    let mut order: Vec<usize> = playing.iter().flatten().copied().collect();
    order.dedup();
    assert!(
        order.len() >= 5 && order.windows(2).all(|pair| pair[1] == (pair[0] + 1) % 3),
        "the frames come round in order: {order:?}"
    );
    let share = |which: usize| playing.iter().filter(|seen| **seen == Some(which)).count();
    assert!(
        share(2) > share(1) && share(1) > share(0),
        "each stays as long as the file said (red 50 ms, green 200, blue 400): {} {} {}",
        share(0),
        share(1),
        share(2)
    );

    harness.send(Input::key(ShortcutKey::Space));
    harness.advance(Duration::from_millis(50));
    let held = frame(&mut harness);
    assert!(held.is_some());
    let paused = watch(
        &mut harness,
        Duration::from_millis(100),
        Duration::from_millis(1500),
    );
    assert!(
        paused.iter().all(|seen| *seen == held),
        "Space holds the frame: {held:?} then {paused:?}"
    );

    harness.send(Input::key(ShortcutKey::Space));
    let again = watch(
        &mut harness,
        Duration::from_millis(50),
        Duration::from_millis(1000),
    );
    let distinct: std::collections::BTreeSet<_> = again.iter().flatten().collect();
    assert!(distinct.len() >= 2, "it plays again: {again:?}");
}

#[test]
fn a_still_picture_has_no_play_button() {
    let (_dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.send(Input::pointer_move(centre()));
    harness.advance(Duration::from_millis(400));
    assert!(harness.centre("[aria-label=\"Pause\"]").is_none());
    assert!(harness.centre("[aria-label=\"Play\"]").is_none());
}
