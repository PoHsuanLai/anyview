//! Zoom arithmetic shared by the stages that zoom. Scales are `Permille` of actual size, so 1000
//! is 1:1; everything is integer.

use anyview_core::{DocPoint, DocUnit, Permille, Zoom};

/// What the view reports of the content as drawn now: the machine decides the next zoom, but
/// only the view knows what "fit" works out to for this window and this content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// The scale on screen now.
    pub shown: Permille,
    /// The scale at which the content fits the window.
    pub fit: Permille,
}

/// Which way a zoom step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDir {
    /// Larger.
    In,
    /// Smaller.
    Out,
}

/// The zoom one step from the scale shown: `step` is the factor in thousandths (1250 is x1.25).
/// A step out that reaches the fit scale is the fit zoom itself, so zooming out is a way back to
/// it; every other result is clamped to the zoom limits.
pub fn stepped(viewport: Viewport, dir: ZoomDir, step: Permille) -> Zoom {
    let shown = u64::from(viewport.shown.0.max(1));
    let step = u64::from(step.0.max(1001));
    let target = match dir {
        ZoomDir::In => shown * step / 1000,
        ZoomDir::Out => shown * 1000 / step,
    };
    let target = Permille(u32::try_from(target).unwrap_or(u32::MAX));
    match dir {
        ZoomDir::Out if target <= viewport.fit => Zoom::Fit,
        ZoomDir::In | ZoomDir::Out => Zoom::scaled(target),
    }
}

/// The scale `zoom` draws at.
pub fn scale_of(zoom: Zoom, viewport: Viewport) -> Permille {
    match zoom {
        Zoom::Fit => viewport.fit,
        Zoom::Fill => viewport.shown,
        Zoom::Actual => Permille::WHOLE,
        Zoom::Scale(scale) => scale,
    }
}

/// The view centre after the scale changes from `old` to `new` with the content point `at` held
/// still on screen: the point's offset from the centre shrinks or grows by the ratio of scales.
pub fn centre_about(at: DocPoint, centre: DocPoint, old: Permille, new: Permille) -> DocPoint {
    let (old, new) = (i64::from(old.0.max(1)), i64::from(new.0.max(1)));
    let axis = |at: DocUnit, centre: DocUnit| {
        let held = i64::from(at.0) - (i64::from(at.0) - i64::from(centre.0)) * old / new;
        DocUnit(i32::try_from(held).unwrap_or(if held < 0 { i32::MIN } else { i32::MAX }))
    };
    DocPoint {
        x: axis(at.x, centre.x),
        y: axis(at.y, centre.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Viewport = Viewport {
        shown: Permille(500),
        fit: Permille(500),
    };
    const STEP: Permille = Permille(1250);

    #[test]
    fn a_step_scales_by_the_factor_and_returns_to_fit() {
        // name, viewport, direction, zoom
        const CASES: &[(&str, Viewport, ZoomDir, Zoom)] = &[
            ("in from fit", VIEW, ZoomDir::In, Zoom::Scale(Permille(625))),
            (
                "in from actual",
                Viewport {
                    shown: Permille(1000),
                    fit: Permille(500),
                },
                ZoomDir::In,
                Zoom::Scale(Permille(1250)),
            ),
            (
                "out above fit",
                Viewport {
                    shown: Permille(1000),
                    fit: Permille(500),
                },
                ZoomDir::Out,
                Zoom::Scale(Permille(800)),
            ),
            (
                "out that reaches fit is fit",
                Viewport {
                    shown: Permille(600),
                    fit: Permille(500),
                },
                ZoomDir::Out,
                Zoom::Fit,
            ),
            ("out at fit stays fit", VIEW, ZoomDir::Out, Zoom::Fit),
            (
                "in is clamped to the top",
                Viewport {
                    shown: Permille(64_000),
                    fit: Permille(500),
                },
                ZoomDir::In,
                Zoom::Scale(Zoom::MAX_SCALE),
            ),
        ];
        for (name, viewport, dir, want) in CASES {
            assert_eq!(stepped(*viewport, *dir, STEP), *want, "{name}");
        }
    }

    #[test]
    fn the_held_point_stays_under_the_pointer() {
        let at = DocPoint {
            x: DocUnit(6400),
            y: DocUnit(0),
        };
        let centre = DocPoint {
            x: DocUnit(3200),
            y: DocUnit(0),
        };
        // Doubling the scale halves the point's distance from the centre.
        let doubled = centre_about(at, centre, Permille(500), Permille(1000));
        assert_eq!(doubled.x, DocUnit(4800));
        // Halving it doubles the distance.
        let halved = centre_about(at, centre, Permille(1000), Permille(500));
        assert_eq!(halved.x, DocUnit(0));
        // A point at the centre moves nothing.
        assert_eq!(
            centre_about(centre, centre, Permille(500), Permille(2000)),
            centre
        );
    }
}
