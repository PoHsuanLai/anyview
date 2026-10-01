use super::super::zoom::{Viewport, ZoomDir};
use super::*;
use anyview_core::{DocPoint, DocUnit, Permille, QuarterTurn, Resume, Zoom};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use std::num::NonZeroU32;

const fn pt(x: i32, y: i32) -> DocPoint {
    DocPoint {
        x: DocUnit(x),
        y: DocUnit(y),
    }
}
/// The view as drawn: scale shown, scale that fits, content point at the middle.
const fn view(shown: u32, fit: u32, centre: DocPoint) -> RasterParams {
    RasterParams {
        viewport: Viewport {
            shown: Permille(shown),
            fit: Permille(fit),
        },
        centre,
        step: Permille(1250),
    }
}
const fn frames(count: u32) -> FrameCount {
    match NonZeroU32::new(count) {
        Some(count) => FrameCount(count),
        None => panic!("a frame count is at least one"),
    }
}
const fn scale(permille: u32) -> Zoom {
    Zoom::Scale(Permille(permille))
}
const fn fitted(turn: QuarterTurn) -> RasterStage {
    RasterStage::Fitted {
        turn,
        anim: Animation::Still,
    }
}
const fn zoomed(zoom: Zoom, centre: DocPoint) -> RasterStage {
    RasterStage::Zoomed {
        turn: QuarterTurn::None,
        zoom,
        centre,
        anim: Animation::Still,
    }
}
const fn panning(centre: DocPoint) -> RasterStage {
    RasterStage::Panning {
        turn: QuarterTurn::None,
        zoom: scale(1000),
        centre,
        anim: Animation::Still,
    }
}
const fn fitted_playing(frame: u32, of: u32) -> RasterStage {
    RasterStage::Fitted {
        turn: QuarterTurn::None,
        anim: Animation::Playing {
            frame: FrameIndex(frame),
            of: frames(of),
        },
    }
}
const fn fitted_paused(frame: u32, of: u32) -> RasterStage {
    RasterStage::Fitted {
        turn: QuarterTurn::None,
        anim: Animation::Paused {
            frame: FrameIndex(frame),
            of: frames(of),
        },
    }
}
const fn remember(zoom: Zoom, centre: DocPoint) -> RasterOut {
    RasterOut::Remember(Resume::Raster { zoom, centre })
}

const HALF: RasterParams = view(500, 500, pt(0, 0));
const ACTUAL: RasterParams = view(1000, 500, pt(3200, 0));

/// Name, what the view shows, state before, input, state after, outputs.
type Case = (
    &'static str,
    RasterParams,
    RasterStage,
    RasterIn,
    RasterStage,
    &'static [RasterOut],
);

const CASES: &[Case] = &[
    (
        "a zoom step in from fit about the centre",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at: pt(0, 0),
        },
        zoomed(scale(625), pt(0, 0)),
        &[remember(scale(625), pt(0, 0))],
    ),
    (
        "a zoom step in holds the point under the pointer",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at: pt(6400, 0),
        },
        zoomed(scale(625), pt(1280, 0)),
        &[remember(scale(625), pt(1280, 0))],
    ),
    (
        "a zoom step out above fit stays zoomed",
        ACTUAL,
        zoomed(Zoom::Actual, pt(3200, 0)),
        RasterIn::ZoomStep {
            dir: ZoomDir::Out,
            at: pt(3200, 0),
        },
        zoomed(scale(800), pt(3200, 0)),
        &[remember(scale(800), pt(3200, 0))],
    ),
    (
        "a zoom step out that reaches fit refits",
        view(600, 500, pt(0, 0)),
        zoomed(scale(600), pt(0, 0)),
        RasterIn::ZoomStep {
            dir: ZoomDir::Out,
            at: pt(0, 0),
        },
        fitted(QuarterTurn::None),
        &[remember(Zoom::Fit, pt(0, 0))],
    ),
    (
        "double click on a fitted image goes to 1:1 at the point",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::DoubleClick { at: pt(6400, 0) },
        zoomed(Zoom::Actual, pt(3200, 0)),
        &[remember(Zoom::Actual, pt(3200, 0))],
    ),
    (
        "double click on a zoomed image goes back to fit",
        ACTUAL,
        zoomed(Zoom::Actual, pt(3200, 0)),
        RasterIn::DoubleClick { at: pt(100, 100) },
        fitted(QuarterTurn::None),
        &[remember(Zoom::Fit, pt(0, 0))],
    ),
    (
        "setting fit refits",
        ACTUAL,
        zoomed(Zoom::Actual, pt(3200, 0)),
        RasterIn::SetZoom {
            zoom: Zoom::Fit,
            at: pt(0, 0),
        },
        fitted(QuarterTurn::None),
        &[remember(Zoom::Fit, pt(0, 0))],
    ),
    (
        "setting a scale past the limit clamps it",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::SetZoom {
            zoom: scale(100_000),
            at: pt(0, 0),
        },
        zoomed(Zoom::Scale(Zoom::MAX_SCALE), pt(0, 0)),
        &[remember(Zoom::Scale(Zoom::MAX_SCALE), pt(0, 0))],
    ),
    (
        "a drag starts on a zoomed image",
        ACTUAL,
        zoomed(scale(1000), pt(3200, 0)),
        RasterIn::PanStart,
        panning(pt(3200, 0)),
        &[],
    ),
    (
        "a drag on a fitted image has nothing to move",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::PanStart,
        fitted(QuarterTurn::None),
        &[],
    ),
    (
        "the content follows the pointer",
        ACTUAL,
        panning(pt(3200, 3200)),
        RasterIn::PanBy(pt(640, -320)),
        panning(pt(2560, 3520)),
        &[],
    ),
    (
        "the drag ends zoomed and remembers where",
        ACTUAL,
        panning(pt(2560, 3520)),
        RasterIn::PanEnd,
        zoomed(scale(1000), pt(2560, 3520)),
        &[remember(scale(1000), pt(2560, 3520))],
    ),
    (
        "a zoom step during a drag is ignored",
        ACTUAL,
        panning(pt(0, 0)),
        RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at: pt(0, 0),
        },
        panning(pt(0, 0)),
        &[],
    ),
    (
        "rotating right turns a quarter and says so",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::Rotate(Spin::Right),
        fitted(QuarterTurn::Quarter),
        &[
            RasterOut::Turned(QuarterTurn::Quarter),
            remember(Zoom::Fit, pt(0, 0)),
        ],
    ),
    (
        "rotating left from none is three quarters",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::Rotate(Spin::Left),
        fitted(QuarterTurn::ThreeQuarter),
        &[
            RasterOut::Turned(QuarterTurn::ThreeQuarter),
            remember(Zoom::Fit, pt(0, 0)),
        ],
    ),
    (
        "rotating a zoomed image refits it",
        ACTUAL,
        zoomed(Zoom::Actual, pt(3200, 0)),
        RasterIn::Rotate(Spin::Right),
        fitted(QuarterTurn::Quarter),
        &[
            RasterOut::Turned(QuarterTurn::Quarter),
            remember(Zoom::Fit, pt(0, 0)),
        ],
    ),
    (
        "restoring a stored scale zooms to it",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::Restore {
            zoom: scale(2000),
            centre: pt(640, 640),
        },
        zoomed(scale(2000), pt(640, 640)),
        &[],
    ),
    (
        "restoring a stored scale clamps it",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::Restore {
            zoom: scale(0),
            centre: pt(0, 0),
        },
        zoomed(Zoom::Scale(Zoom::MIN_SCALE), pt(0, 0)),
        &[],
    ),
    (
        "restoring fit stays fitted",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::Restore {
            zoom: Zoom::Fit,
            centre: pt(0, 0),
        },
        fitted(QuarterTurn::None),
        &[],
    ),
    (
        "an animated file starts playing on frame zero",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::Animated(frames(3)),
        fitted_playing(0, 3),
        &[RasterOut::ShowFrame(FrameIndex(0))],
    ),
    (
        "a tick shows the next frame",
        HALF,
        fitted_playing(0, 3),
        RasterIn::FrameTick,
        fitted_playing(1, 3),
        &[RasterOut::ShowFrame(FrameIndex(1))],
    ),
    (
        "a tick on the last frame wraps",
        HALF,
        fitted_playing(2, 3),
        RasterIn::FrameTick,
        fitted_playing(0, 3),
        &[RasterOut::ShowFrame(FrameIndex(0))],
    ),
    (
        "toggling pauses on the frame",
        HALF,
        fitted_playing(1, 3),
        RasterIn::TogglePlayback,
        fitted_paused(1, 3),
        &[],
    ),
    (
        "a paused animation ignores ticks",
        HALF,
        fitted_paused(1, 3),
        RasterIn::FrameTick,
        fitted_paused(1, 3),
        &[],
    ),
    (
        "toggling resumes",
        HALF,
        fitted_paused(1, 3),
        RasterIn::TogglePlayback,
        fitted_playing(1, 3),
        &[],
    ),
    (
        "a still image has nothing to toggle",
        HALF,
        fitted(QuarterTurn::None),
        RasterIn::TogglePlayback,
        fitted(QuarterTurn::None),
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, params, from, input, state, outs) in CASES {
        let (next, out) = from.step(*input, Stamp(0), params);
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn zooming_keeps_the_animation_it_found() {
    let playing = RasterStage::Fitted {
        turn: QuarterTurn::None,
        anim: Animation::Playing {
            frame: FrameIndex(2),
            of: frames(5),
        },
    };
    let (next, _) = playing.step(
        RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at: pt(0, 0),
        },
        Stamp(0),
        &HALF,
    );
    let RasterStage::Zoomed { anim, .. } = next else {
        panic!("a zoom step from fit zooms");
    };
    assert_eq!(
        anim,
        Animation::Playing {
            frame: FrameIndex(2),
            of: frames(5)
        }
    );
}
