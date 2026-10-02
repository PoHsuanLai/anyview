//! The launch budget (PLAN section 7, phase B): cold, the first content of a JPEG within 150 ms of
//! the process starting; warm, within 50 ms of asking the running viewer for the next file.
//!
//! These are ignored tests, because the numbers mean something only in a release build on a quiet
//! machine:
//!
//! `cargo test --release -p anyview --test launch -- --ignored --nocapture --test-threads=1`
//!
//! What is measured is the closest the harness can see. The harness is a real Blitz document on
//! a real wgpu device (no window and no compositor), so the cold figure runs from the start of the
//! program's own wiring (the runtime, the pool, the desktop, the folder listing) through opening
//! the document and its GPU device, the probe and decode on the pool, the upload into the texture
//! and the picture's element appearing, to the first frame drawn with it. It leaves out the
//! executable's start and the compositor's present, which the third test (the bare process) puts a
//! number on. The warm figure is the arrow key in an open window: the next file's probe, decode,
//! upload and first frame drawn, on a pool and a device that are already running.

#![allow(clippy::unwrap_used)]

mod support;

use ds::prelude::ShortcutKey;
use ds_harness::{Driver, Input, Query};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use support::{Rig, fixture, open, until};

/// The most the cold open may take, in milliseconds. A ratchet: lower it when a run beats it, and
/// never raise it (FINDINGS, "The launch budget"). The target is 150; the window's own start-up
/// (the GPU device, the fonts, the first layout) is most of what is above it.
const COLD_BUDGET_MS: u128 = 300;
/// The most the warm open may take, in milliseconds; the same ratchet, with a target of 50.
const WARM_BUDGET_MS: u128 = 15;
/// The most the bare process may take to start, answer `--help` and exit, in milliseconds.
const PROCESS_BUDGET_MS: u128 = 5;

fn folder() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("1-quadrants.png");
    let second = dir.path().join("2-plain.jpg");
    std::fs::copy(fixture("anyview-image", "quadrants.png"), &first).unwrap();
    std::fs::copy(fixture("anyview-image", "plain.jpg"), &second).unwrap();
    (dir, first, second)
}

/// The picture is in the document: the stage made its texture layer.
fn picture_is_up(harness: &mut ds_harness::Harness) -> bool {
    harness.count(".viewer-raster-picture") > 0
}

fn milliseconds(span: Duration) -> f64 {
    span.as_secs_f64() * 1000.0
}

#[test]
#[ignore = "a release build on a quiet machine; see the module documentation"]
fn cold_first_content_of_a_jpeg_is_within_the_budget() {
    let (dir, _, jpeg) = folder();
    let mut rig: Rig = open(&jpeg, dir.path());
    until(&mut rig.harness, "the picture", picture_is_up);
    let ready = rig.started.elapsed();
    let frame = Instant::now();
    rig.harness.render().unwrap();
    let drawn = rig.started.elapsed();
    println!(
        "cold jpeg: wiring {:.1} ms, window up at {:.1} ms, picture ready at {:.1} ms, \
         first frame drawn at {:.1} ms (the frame itself {:.1} ms)",
        milliseconds(rig.wired),
        milliseconds(rig.windowed),
        milliseconds(ready),
        milliseconds(drawn),
        milliseconds(frame.elapsed()),
    );
    assert!(
        drawn.as_millis() <= COLD_BUDGET_MS,
        "cold first content took {} ms, the budget is {COLD_BUDGET_MS} ms",
        drawn.as_millis()
    );
}

#[test]
#[ignore = "a release build on a quiet machine; see the module documentation"]
fn a_window_with_nothing_in_it_is_the_floor_of_the_cold_figure() {
    use dioxus::prelude::*;
    use ds_harness::{Backend, Clock, Harness, HarnessConfig};
    fn nothing() -> Element {
        rsx! {}
    }
    let started = Instant::now();
    let config = HarnessConfig::new(support::VIEW)
        .with_clock(Clock::Virtual)
        .with_backend(Backend::Hybrid);
    let _harness = Harness::new(nothing, config);
    println!(
        "empty window: up at {:.1} ms (the GPU device, the fonts and the first frame, with nothing of ours)",
        milliseconds(started.elapsed())
    );
}

#[test]
#[ignore = "a release build on a quiet machine; see the module documentation"]
fn warm_next_file_is_within_the_budget() {
    let (dir, png, _) = folder();
    let mut rig = open(&png, dir.path());
    until(&mut rig.harness, "the first picture", picture_is_up);
    let before = rig.harness.render().unwrap();

    let asked = Instant::now();
    rig.harness.send(Input::key(ShortcutKey::Right));
    until(&mut rig.harness, "the next file's frame", |harness| {
        harness.render().unwrap().as_raw() != before.as_raw()
    });
    let warm = asked.elapsed();
    println!(
        "warm next file: first frame of the next picture {:.1} ms",
        milliseconds(warm)
    );
    assert!(
        warm.as_millis() <= WARM_BUDGET_MS,
        "warm open took {} ms, the budget is {WARM_BUDGET_MS} ms",
        warm.as_millis()
    );
}

#[test]
#[ignore = "a release build on a quiet machine; see the module documentation"]
fn the_bare_process_starts_and_exits_within_the_budget() {
    const RUNS: usize = 21;
    let binary = env!("CARGO_BIN_EXE_anyview");
    let mut spans: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let status = Command::new(binary)
                .arg("--help")
                .stdout(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success());
            started.elapsed()
        })
        .collect();
    spans.sort();
    let median = spans[RUNS / 2];
    println!(
        "bare process (--help): median {:.1} ms, best {:.1} ms, worst {:.1} ms over {RUNS} runs",
        milliseconds(median),
        milliseconds(spans[0]),
        milliseconds(spans[RUNS - 1]),
    );
    assert!(
        median.as_millis() <= PROCESS_BUDGET_MS,
        "the bare process took {} ms, the budget is {PROCESS_BUDGET_MS} ms",
        median.as_millis()
    );
}
