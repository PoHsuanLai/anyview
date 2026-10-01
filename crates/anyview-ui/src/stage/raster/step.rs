//! The raster stage's transitions.

use super::super::zoom::{centre_about, scale_of, stepped};
use super::model::{Animation, FrameIndex, RasterIn, RasterOut, RasterParams, RasterStage, Spin};
use anyview_core::{DocPoint, DocUnit, QuarterTurn, Resume, Zoom};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (RasterStage, Vec<RasterOut>);

impl Machine for RasterStage {
    type In = RasterIn;
    type Out = RasterOut;
    type Params = RasterParams;

    fn step(self, input: RasterIn, _at: Stamp, params: &RasterParams) -> Step {
        match self {
            RasterStage::Fitted { turn, anim } => fitted(self, turn, anim, input, params),
            RasterStage::Zoomed {
                turn,
                zoom,
                centre,
                anim,
            } => zoomed(self, (turn, zoom, centre, anim), input, params),
            RasterStage::Panning {
                turn,
                zoom,
                centre,
                anim,
            } => panning(self, (turn, zoom, centre, anim), input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            RasterStage::Fitted { turn: _, anim: _ }
            | RasterStage::Zoomed {
                turn: _,
                zoom: _,
                centre: _,
                anim: _,
            }
            | RasterStage::Panning {
                turn: _,
                zoom: _,
                centre: _,
                anim: _,
            } => None,
        }
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

fn rotated(turn: QuarterTurn, spin: Spin) -> QuarterTurn {
    match spin {
        Spin::Left => turn.then(QuarterTurn::ThreeQuarter),
        Spin::Right => turn.then(QuarterTurn::Quarter),
    }
}

/// Rotating refits: a centre in the old orientation means nothing in the new one.
fn rotate_fitted(turn: QuarterTurn, anim: Animation, spin: Spin) -> Step {
    let turn = rotated(turn, spin);
    (
        RasterStage::Fitted { turn, anim },
        vec![
            RasterOut::Turned(turn),
            remembered(Zoom::Fit, DocPoint::default()),
        ],
    )
}

fn animated(anim: Animation, input: RasterIn) -> Option<(Animation, Vec<RasterOut>)> {
    match (anim, input) {
        (Animation::Still, RasterIn::Animated(of)) => {
            let anim = Animation::Playing {
                frame: FrameIndex(0),
                of,
            };
            Some((anim, vec![RasterOut::ShowFrame(FrameIndex(0))]))
        }
        (Animation::Playing { frame, of }, RasterIn::FrameTick) => {
            let frame = FrameIndex((frame.0 + 1) % of.0.get());
            Some((
                Animation::Playing { frame, of },
                vec![RasterOut::ShowFrame(frame)],
            ))
        }
        (Animation::Playing { frame, of }, RasterIn::TogglePlayback) => {
            Some((Animation::Paused { frame, of }, vec![]))
        }
        (Animation::Paused { frame, of }, RasterIn::TogglePlayback) => {
            Some((Animation::Playing { frame, of }, vec![]))
        }
        (
            Animation::Still
            | Animation::Playing { frame: _, of: _ }
            | Animation::Paused { frame: _, of: _ },
            _,
        ) => None,
    }
}

fn fitted(
    this: RasterStage,
    turn: QuarterTurn,
    anim: Animation,
    input: RasterIn,
    params: &RasterParams,
) -> Step {
    match input {
        RasterIn::ZoomStep { dir, at } => {
            let zoom = stepped(params.viewport, dir, params.step);
            zoomed_to(turn, anim, zoom, at, params)
        }
        RasterIn::SetZoom { zoom, at } => zoomed_to(turn, anim, clamped(zoom), at, params),
        RasterIn::DoubleClick { at } => zoomed_to(turn, anim, Zoom::Actual, at, params),
        RasterIn::Rotate(spin) => rotate_fitted(turn, anim, spin),
        RasterIn::Restore { zoom, centre } => restored(turn, anim, zoom, centre),
        RasterIn::Animated(_) | RasterIn::FrameTick | RasterIn::TogglePlayback => {
            match animated(anim, input) {
                Some((anim, outs)) => (RasterStage::Fitted { turn, anim }, outs),
                None => (this, vec![]),
            }
        }
        RasterIn::PanStart | RasterIn::PanBy(_) | RasterIn::PanEnd | RasterIn::Elapsed => {
            (this, vec![])
        }
    }
}

fn zoomed(this: RasterStage, view: View, input: RasterIn, params: &RasterParams) -> Step {
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
        RasterIn::Rotate(spin) => rotate_fitted(turn, anim, spin),
        RasterIn::Restore { zoom, centre } => restored(turn, anim, zoom, centre),
        RasterIn::Animated(_) | RasterIn::FrameTick | RasterIn::TogglePlayback => {
            match animated(anim, input) {
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
            }
        }
        RasterIn::PanBy(_) | RasterIn::PanEnd | RasterIn::Elapsed => (this, vec![]),
    }
}

fn panning(this: RasterStage, view: View, input: RasterIn) -> Step {
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
        RasterIn::Animated(_) | RasterIn::FrameTick | RasterIn::TogglePlayback => {
            match animated(anim, input) {
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
            }
        }
        RasterIn::ZoomStep { dir: _, at: _ }
        | RasterIn::SetZoom { zoom: _, at: _ }
        | RasterIn::DoubleClick { at: _ }
        | RasterIn::PanStart
        | RasterIn::Rotate(_)
        | RasterIn::Restore { zoom: _, centre: _ }
        | RasterIn::Elapsed => (this, vec![]),
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
