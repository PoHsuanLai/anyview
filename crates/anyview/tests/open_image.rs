//! A picture opened end to end under the harness with the binary's own wiring: the first window's
//! root, its work on the runtime's pool, its request to the desktop. The pixels are read back from
//! what the window drew.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_store::{HistoryRead, read_history};
use ds_harness::{Driver, Query};
use support::{fixture, open, until};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const YELLOW: [u8; 3] = [255, 255, 0];

fn is(got: image::Rgba<u8>, want: [u8; 3]) -> bool {
    got.0
        .iter()
        .zip(want)
        .all(|(got, want)| (i32::from(*got) - i32::from(want)).abs() < 40)
}

#[test]
fn a_picture_is_decoded_on_the_pool_drawn_in_the_window_and_recorded_as_viewed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("quadrants.png");
    std::fs::copy(fixture("anyview-image", "quadrants.png"), &file).unwrap();
    let mut rig = open(&file, dir.path());

    until(&mut rig.harness, "the picture's stage", |harness| {
        harness.count(".viewer-raster") > 0
    });
    let mut picture = None;
    until(&mut rig.harness, "the picture's pixels", |harness| {
        let drawn = harness.render().unwrap();
        let (x, y) = (drawn.width() / 2, drawn.height() / 2);
        let seen = is(*drawn.get_pixel(x - 12, y - 8), RED);
        picture = Some(drawn);
        seen
    });
    let drawn = picture.unwrap();
    let (x, y) = (drawn.width() / 2, drawn.height() / 2);
    for (name, dx, dy, want) in [
        ("top left", -12_i64, -8_i64, RED),
        ("top right", 12, -8, GREEN),
        ("bottom left", -12, 8, BLUE),
        ("bottom right", 12, 8, YELLOW),
    ] {
        let at = (
            u32::try_from(i64::from(x) + dx).unwrap(),
            u32::try_from(i64::from(y) + dy).unwrap(),
        );
        let got = *drawn.get_pixel(at.0, at.1);
        assert!(is(got, want), "{name}: {got:?} is not {want:?}");
    }

    until(&mut rig.harness, "the view recorded", |_| {
        matches!(read_history(&rig.store), HistoryRead::Loaded(history)
            if history.entries.iter().any(|entry| entry.path.as_path() == file))
    });
    assert!(
        rig.workforce.notices().is_empty(),
        "every job ran to its end and posted its result"
    );
}
