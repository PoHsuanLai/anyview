//! The animation a stage carries: frame times on the virtual clock, runs, pause, step and motion.

use super::super::*;
use crate::stage::media::StepDirection;
use crate::testing::settle;
use anyview_core::QuarterTurn;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use std::num::NonZeroU32;
use std::time::Duration;

const fn frames(count: u32) -> FrameCount {
    match NonZeroU32::new(count) {
        Some(count) => FrameCount(count),
        None => panic!("a frame count is at least one"),
    }
}

/// Three frames that stay 50, 200 and 400 ms.
fn params(runs: Runs, motion: Motion) -> RasterParams {
    let delays: Vec<Duration> = [50, 200, 400].map(Duration::from_millis).to_vec();
    RasterParams {
        delays: FrameDelays(delays.into()),
        runs,
        motion,
        ..RasterParams::default()
    }
}

fn forever() -> RasterParams {
    params(Runs::Forever, Motion::Standard)
}

fn with(anim: Animation) -> RasterStage {
    RasterStage::Fitted {
        turn: QuarterTurn::None,
        anim,
    }
}

fn playing(frame: u32, due: u64, run: u32) -> RasterStage {
    with(Animation::Playing {
        frame: FrameIndex(frame),
        of: frames(3),
        due: Stamp(due),
        run,
    })
}

fn paused(frame: u32, run: u32) -> RasterStage {
    with(Animation::Paused {
        frame: FrameIndex(frame),
        of: frames(3),
        run,
    })
}

fn ended(frame: u32) -> RasterStage {
    with(Animation::Ended {
        frame: FrameIndex(frame),
        of: frames(3),
    })
}

fn show(frame: u32) -> RasterOut {
    RasterOut::ShowFrame(FrameIndex(frame))
}

/// Name, state before, input, time, state after, outputs, next wake.
type Case = (
    &'static str,
    fn() -> RasterStage,
    RasterIn,
    u64,
    fn() -> RasterStage,
    fn() -> Vec<RasterOut>,
    Option<Stamp>,
);

#[test]
fn every_row_of_the_table_steps_as_written() {
    #[rustfmt::skip]
    let cases: &[Case] = &[
        ("an animated file starts on frame zero and waits its delay",
            || with(Animation::Still), RasterIn::Animated(frames(3)), 1000,
            || playing(0, 1050, 0), || vec![show(0)], Some(Stamp(1050))),
        ("the time before the delay is up changes nothing",
            || playing(0, 1050, 0), RasterIn::Elapsed, 1049,
            || playing(0, 1050, 0), Vec::new, Some(Stamp(1050))),
        ("the time at the delay shows the next frame and waits its delay",
            || playing(0, 1050, 0), RasterIn::Elapsed, 1050,
            || playing(1, 1250, 0), || vec![show(1)], Some(Stamp(1250))),
        ("a late wake does not make the next frame early",
            || playing(1, 1250, 0), RasterIn::Elapsed, 1300,
            || playing(2, 1700, 0), || vec![show(2)], Some(Stamp(1700))),
        ("the last frame wraps to the first, a run later",
            || playing(2, 1700, 0), RasterIn::Elapsed, 1700,
            || playing(0, 1750, 1), || vec![show(0)], Some(Stamp(1750))),
        ("toggling pauses on the frame and stops the clock",
            || playing(1, 1250, 0), RasterIn::TogglePlayback, 1100,
            || paused(1, 0), Vec::new, None),
        ("a paused animation ignores the clock",
            || paused(1, 0), RasterIn::Elapsed, 9000,
            || paused(1, 0), Vec::new, None),
        ("toggling resumes with the frame's whole delay",
            || paused(1, 0), RasterIn::TogglePlayback, 2000,
            || playing(1, 2200, 0), Vec::new, Some(Stamp(2200))),
        ("a step forward pauses on the next frame",
            || playing(0, 1050, 0), RasterIn::StepFrame(StepDirection::Forward), 1010,
            || paused(1, 0), || vec![show(1)], None),
        ("a step forward from the last frame wraps",
            || paused(2, 0), RasterIn::StepFrame(StepDirection::Forward), 0,
            || paused(0, 0), || vec![show(0)], None),
        ("a step back from the first frame wraps",
            || paused(0, 0), RasterIn::StepFrame(StepDirection::Backward), 0,
            || paused(2, 0), || vec![show(2)], None),
        ("a step back goes to the frame before",
            || paused(2, 0), RasterIn::StepFrame(StepDirection::Backward), 0,
            || paused(1, 0), || vec![show(1)], None),
        ("toggling an ended animation plays it again from the start",
            || ended(2), RasterIn::TogglePlayback, 5000,
            || playing(0, 5050, 0), || vec![show(0)], Some(Stamp(5050))),
        ("a step from an ended animation pauses on the frame before",
            || ended(2), RasterIn::StepFrame(StepDirection::Backward), 5000,
            || paused(1, 0), || vec![show(1)], None),
        ("a still image has nothing to toggle",
            || with(Animation::Still), RasterIn::TogglePlayback, 0,
            || with(Animation::Still), Vec::new, None),
        ("a still image has nothing to step",
            || with(Animation::Still), RasterIn::StepFrame(StepDirection::Forward), 0,
            || with(Animation::Still), Vec::new, None),
    ];
    for (name, from, input, at, state, outs, wake) in cases {
        let (next, out) = from().step(*input, Stamp(*at), &forever(), &());
        assert_eq!(next, state(), "{name}: state");
        assert_eq!(out, outs(), "{name}: outputs");
        assert_eq!(next.wake(), *wake, "{name}: next wake");
    }
}

#[test]
fn a_run_of_the_virtual_clock_shows_each_frame_at_its_time() {
    let (start, _) =
        with(Animation::Still).step(RasterIn::Animated(frames(3)), Stamp(0), &forever(), &());
    let (_, log) = settle(start, &forever(), &(), 7);
    let want: Vec<(Stamp, RasterOut)> = [
        (50, 1),
        (250, 2),
        (650, 0),
        (700, 1),
        (900, 2),
        (1300, 0),
        (1350, 1),
    ]
    .map(|(at, frame)| (Stamp(at), show(frame)))
    .to_vec();
    assert_eq!(log, want);
}

#[test]
fn the_runs_a_file_asks_for_are_played_and_the_last_frame_stays() {
    let counted = params(Runs::Times(NonZeroU32::new(2).unwrap()), Motion::Standard);
    let (start, _) =
        with(Animation::Still).step(RasterIn::Animated(frames(3)), Stamp(0), &counted, &());
    let (rest, log) = settle(start, &counted, &(), 50);
    let shown: Vec<u32> = log
        .iter()
        .map(|(_, out)| match out {
            RasterOut::ShowFrame(frame) => frame.0,
            RasterOut::Remember(_) => panic!("playing remembers nothing"),
        })
        .collect();
    assert_eq!(
        shown,
        [1, 2, 0, 1, 2],
        "two runs of three frames, the first already shown"
    );
    assert_eq!(rest, ended(2));
    assert_eq!(rest.wake(), None, "an ended animation keeps no timer");
    assert_eq!(
        log.last().map(|(at, _)| *at),
        Some(Stamp(50 + 200 + 400 + 50 + 200))
    );
}

#[test]
fn reduced_motion_opens_an_animation_paused_on_its_first_frame() {
    let reduced = params(Runs::Forever, Motion::Reduced);
    let (next, out) =
        with(Animation::Still).step(RasterIn::Animated(frames(3)), Stamp(0), &reduced, &());
    assert_eq!(next, paused(0, 0));
    assert_eq!(out, vec![show(0)]);
    assert_eq!(next.wake(), None);
    let (played, _) = next.step(RasterIn::TogglePlayback, Stamp(10), &reduced, &());
    assert_eq!(played, playing(0, 60, 0), "the person can still play it");
}

#[test]
fn a_file_without_delays_is_shown_at_a_readable_default() {
    let (next, _) = with(Animation::Still).step(
        RasterIn::Animated(frames(2)),
        Stamp(0),
        &RasterParams::default(),
        &(),
    );
    assert_eq!(next.wake(), Some(Stamp(100)));
}

#[test]
fn zooming_keeps_the_animation_it_found() {
    let before = playing(2, 700, 1);
    let (next, _) = before.step(
        RasterIn::ZoomStep {
            dir: crate::stage::zoom::ZoomDir::In,
            at: anyview_core::DocPoint::default(),
        },
        Stamp(0),
        &forever(),
        &(),
    );
    let RasterStage::Zoomed { anim, .. } = next else {
        panic!("a zoom step from fit zooms");
    };
    assert_eq!(
        anim,
        Animation::Playing {
            frame: FrameIndex(2),
            of: frames(3),
            due: Stamp(700),
            run: 1
        }
    );
    assert_eq!(next.wake(), Some(Stamp(700)), "zoomed, it still ticks");
}
