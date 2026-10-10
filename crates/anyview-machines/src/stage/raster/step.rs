//! The raster stage's transitions.

use super::super::media::StepDirection;
use super::super::zoom::{centre_about, scale_of, stepped};
use super::model::{
    Animation, FrameCount, FrameIndex, Motion, RasterIn, RasterOut, RasterParams, RasterStage, Runs,
};
use crate::time::after;
use anyview_core::{DocPoint, DocUnit, QuarterTurn, Resume, Zoom};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use std::time::Duration;

type Step = (RasterStage, Vec<RasterOut>);

impl Machine for RasterStage {
    type In = RasterIn;
    type Out = RasterOut;
    type Params = RasterParams;
    type Ctx = ();

    fn step(self, input: RasterIn, at: Stamp, params: &RasterParams, _cx: &()) -> Step {
        match self {
            RasterStage::Fitted { turn, anim } => fitted(self, turn, anim, input, (at, params)),
            RasterStage::Zoomed {
                turn,
                zoom,
                centre,
                anim,
            } => zoomed(self, (turn, zoom, centre, anim), input, (at, params)),
            RasterStage::Panning {
                turn,
                zoom,
                centre,
                anim,
            } => panning(self, (turn, zoom, centre, anim), input, (at, params)),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match animation(self) {
            Animation::Playing { due, .. } => Some(due),
            Animation::Still | Animation::Paused { .. } | Animation::Ended { .. } => None,
        }
    }
}

fn animation(stage: &RasterStage) -> Animation {
    match stage {
        RasterStage::Fitted { anim, .. }
        | RasterStage::Zoomed { anim, .. }
        | RasterStage::Panning { anim, .. } => *anim,
    }
}

/// A zoomed view: turn, zoom, centre, animation.
type View = (QuarterTurn, Zoom, DocPoint, Animation);

fn remembered(zoom: Zoom, centre: DocPoint) -> RasterOut {
    RasterOut::Remember(Resume::Raster { zoom, centre })
}

/// `zoom` held about `at`, from the scale and centre now on screen. `Fit` is the fitted state.
fn zoomed_to(
    turn: QuarterTurn,
    anim: Animation,
    zoom: Zoom,
    at: DocPoint,
    params: &RasterParams,
) -> Step {
    match zoom {
        Zoom::Fit => (
            RasterStage::Fitted { turn, anim },
            vec![remembered(Zoom::Fit, DocPoint::default())],
        ),
        Zoom::Fill | Zoom::Actual | Zoom::Scale(_) => {
            let old = params.viewport.shown;
            let new = scale_of(zoom, params.viewport);
            let centre = centre_about(at, params.centre, old, new);
            let state = RasterStage::Zoomed {
                turn,
                zoom,
                centre,
                anim,
            };
            (state, vec![remembered(zoom, centre)])
        }
    }
}

/// What a step needs of the clock and the file: when it is and what the file says.
type Moment<'a> = (Stamp, &'a RasterParams);

/// How long `frame` stays; a file that gave no delays is shown at the browsers' slowest default.
fn stay(params: &RasterParams, frame: FrameIndex) -> Duration {
    params
        .delays
        .0
        .get(frame.0 as usize)
        .copied()
        .unwrap_or(Duration::from_millis(100))
}

fn playing(frame: FrameIndex, of: FrameCount, run: u32, now: Moment) -> Animation {
    Animation::Playing {
        frame,
        of,
        due: after(now.0, stay(now.1, frame)),
        run,
    }
}

/// The frame `dir` of `frame`, wrapping round `of` frames.
fn neighbour(frame: FrameIndex, of: FrameCount, dir: StepDirection) -> FrameIndex {
    let last = of.0.get() - 1;
    FrameIndex(match dir {
        StepDirection::Forward if frame.0 >= last => 0,
        StepDirection::Forward => frame.0 + 1,
        StepDirection::Backward if frame.0 == 0 => last,
        StepDirection::Backward => frame.0 - 1,
    })
}

/// A frame time that is up: the next frame, or the end when the last run has played out.
fn advanced(
    frame: FrameIndex,
    of: FrameCount,
    run: u32,
    now: Moment,
) -> (Animation, Vec<RasterOut>) {
    if frame.0 + 1 < of.0.get() {
        let next = FrameIndex(frame.0 + 1);
        return (
            playing(next, of, run, now),
            vec![RasterOut::ShowFrame(next)],
        );
    }
    let done = run.saturating_add(1);
    match now.1.runs {
        Runs::Times(times) if done >= times.get() => (Animation::Ended { frame, of }, vec![]),
        Runs::Times(_) | Runs::Forever => (
            playing(FrameIndex(0), of, done, now),
            vec![RasterOut::ShowFrame(FrameIndex(0))],
        ),
    }
}

fn animated(anim: Animation, input: RasterIn, now: Moment) -> Option<(Animation, Vec<RasterOut>)> {
    let first = FrameIndex(0);
    let shown = |anim, frame| Some((anim, vec![RasterOut::ShowFrame(frame)]));
    match (anim, input) {
        (Animation::Still, RasterIn::Animated(of)) => match now.1.motion {
            Motion::Standard => shown(playing(first, of, 0, now), first),
            Motion::Reduced => shown(
                Animation::Paused {
                    frame: first,
                    of,
                    run: 0,
                },
                first,
            ),
        },
        (
            Animation::Playing {
                frame,
                of,
                due,
                run,
            },
            RasterIn::Elapsed,
        ) if now.0 >= due => Some(advanced(frame, of, run, now)),
        (Animation::Playing { frame, of, run, .. }, RasterIn::TogglePlayback) => {
            Some((Animation::Paused { frame, of, run }, vec![]))
        }
        (Animation::Paused { frame, of, run }, RasterIn::TogglePlayback) => {
            Some((playing(frame, of, run, now), vec![]))
        }
        (Animation::Ended { frame: _, of }, RasterIn::TogglePlayback) => {
            shown(playing(first, of, 0, now), first)
        }
        (
            Animation::Playing { frame, of, run, .. } | Animation::Paused { frame, of, run },
            RasterIn::StepFrame(dir),
        ) => {
            let next = neighbour(frame, of, dir);
            shown(
                Animation::Paused {
                    frame: next,
                    of,
                    run,
                },
                next,
            )
        }
        (Animation::Ended { frame, of }, RasterIn::StepFrame(dir)) => {
            let next = neighbour(frame, of, dir);
            shown(
                Animation::Paused {
                    frame: next,
                    of,
                    run: 0,
                },
                next,
            )
        }
        (
            Animation::Still
            | Animation::Playing { .. }
            | Animation::Paused { .. }
            | Animation::Ended { .. },
            _,
        ) => None,
    }
}

fn fitted(
    this: RasterStage,
    turn: QuarterTurn,
    anim: Animation,
    input: RasterIn,
    now: Moment,
) -> Step {
    let params = now.1;
    match input {
        RasterIn::ZoomStep { dir, at } => {
            let zoom = stepped(params.viewport, dir, params.step);
            zoomed_to(turn, anim, zoom, at, params)
        }
        RasterIn::SetZoom { zoom, at } => zoomed_to(turn, anim, clamped(zoom), at, params),
        RasterIn::DoubleClick { at } => zoomed_to(turn, anim, Zoom::Actual, at, params),
        RasterIn::Restore { zoom, centre } => restored(turn, anim, zoom, centre),
        RasterIn::Animated(_)
        | RasterIn::Elapsed
        | RasterIn::TogglePlayback
        | RasterIn::StepFrame(_) => match animated(anim, input, now) {
            Some((anim, outs)) => (RasterStage::Fitted { turn, anim }, outs),
            None => (this, vec![]),
        },
        RasterIn::PanStart | RasterIn::PanBy(_) | RasterIn::PanEnd => (this, vec![]),
    }
}

fn zoomed(this: RasterStage, view: View, input: RasterIn, now: Moment) -> Step {
    let params = now.1;
    let (turn, zoom, centre, anim) = view;
    match input {
        RasterIn::ZoomStep { dir, at } => {
            let next = stepped(params.viewport, dir, params.step);
            zoomed_to(turn, anim, next, at, params)
        }
        RasterIn::SetZoom { zoom, at } => zoomed_to(turn, anim, clamped(zoom), at, params),
        RasterIn::DoubleClick { at: _ } => zoomed_to(turn, anim, Zoom::Fit, centre, params),
        RasterIn::PanStart => (
            RasterStage::Panning {
                turn,
                zoom,
                centre,
                anim,
            },
            vec![],
        ),
        RasterIn::Restore { zoom, centre } => restored(turn, anim, zoom, centre),
        RasterIn::Animated(_)
        | RasterIn::Elapsed
        | RasterIn::TogglePlayback
        | RasterIn::StepFrame(_) => match animated(anim, input, now) {
            Some((anim, outs)) => (
                RasterStage::Zoomed {
                    turn,
                    zoom,
                    centre,
                    anim,
                },
                outs,
            ),
            None => (this, vec![]),
        },
        RasterIn::PanBy(_) | RasterIn::PanEnd => (this, vec![]),
    }
}

fn panning(this: RasterStage, view: View, input: RasterIn, now: Moment) -> Step {
    let (turn, zoom, centre, anim) = view;
    match input {
        RasterIn::PanBy(by) => {
            let centre = DocPoint {
                x: DocUnit(centre.x.0.saturating_sub(by.x.0)),
                y: DocUnit(centre.y.0.saturating_sub(by.y.0)),
            };
            (
                RasterStage::Panning {
                    turn,
                    zoom,
                    centre,
                    anim,
                },
                vec![],
            )
        }
        RasterIn::PanEnd => (
            RasterStage::Zoomed {
                turn,
                zoom,
                centre,
                anim,
            },
            vec![remembered(zoom, centre)],
        ),
        RasterIn::Animated(_)
        | RasterIn::Elapsed
        | RasterIn::TogglePlayback
        | RasterIn::StepFrame(_) => match animated(anim, input, now) {
            Some((anim, outs)) => (
                RasterStage::Panning {
                    turn,
                    zoom,
                    centre,
                    anim,
                },
                outs,
            ),
            None => (this, vec![]),
        },
        RasterIn::ZoomStep { dir: _, at: _ }
        | RasterIn::SetZoom { zoom: _, at: _ }
        | RasterIn::DoubleClick { at: _ }
        | RasterIn::PanStart
        | RasterIn::Restore { zoom: _, centre: _ } => (this, vec![]),
    }
}

/// A stored zoom, clamped to the limits: a stored `Scale` loads as written.
fn clamped(zoom: Zoom) -> Zoom {
    match zoom {
        Zoom::Scale(scale) => Zoom::scaled(scale),
        Zoom::Fit | Zoom::Fill | Zoom::Actual => zoom,
    }
}

fn restored(turn: QuarterTurn, anim: Animation, zoom: Zoom, centre: DocPoint) -> Step {
    match clamped(zoom) {
        Zoom::Fit => (RasterStage::Fitted { turn, anim }, vec![]),
        zoom @ (Zoom::Fill | Zoom::Actual | Zoom::Scale(_)) => (
            RasterStage::Zoomed {
                turn,
                zoom,
                centre,
                anim,
            },
            vec![],
        ),
    }
}
