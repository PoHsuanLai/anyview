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

impl Viewport {
    /// The smallest scale a picture is zoomed out to by a step: a tenth of actual size, or a
    /// quarter of the fit scale when that is smaller (a picture far larger than the window), and
    /// never below [`Zoom::MIN_SCALE`].
    #[must_use]
    pub fn floor(&self) -> Permille {
        Permille((self.fit.0 / 4).clamp(Zoom::MIN_SCALE.0, 100))
    }
}

/// Which way a zoom step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDir {
    /// Larger.
    In,
    /// Smaller.
    Out,
}

/// The sizes a text, a table or a tree is drawn at, in thousandths of actual size: half to three
/// times, as TextEdit, Numbers and GNOME Text Editor step their zoom.
const LADDER: [u16; 8] = [500, 750, 1000, 1250, 1500, 2000, 2500, 3000];

/// Where on the ladder actual size is.
const ACTUAL_STEP: usize = 2;

/// How large the lines, rows and glyphs of a text, a table or a tree are drawn. Content that
/// reflows zooms in steps of a ladder (50% to 300%) rather than by the free scale a picture has,
/// so the size is always one the row heights and the glyph widths were drawn for. It cannot hold
/// a size off the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentScale(usize);

impl ContentScale {
    /// Actual size.
    pub const ACTUAL: ContentScale = ContentScale(ACTUAL_STEP);

    /// The step one rung up or down, staying at the end of the ladder when there is no more.
    #[must_use]
    pub const fn stepped(self, dir: ZoomDir) -> ContentScale {
        let last = LADDER.len() - 1;
        ContentScale(match dir {
            ZoomDir::In if self.0 < last => self.0 + 1,
            ZoomDir::In => last,
            ZoomDir::Out => self.0.saturating_sub(1),
        })
    }

    /// The size in thousandths of actual (1000 is actual size).
    #[must_use]
    pub fn permille(self) -> Permille {
        Permille(u32::from(
            LADDER.get(self.0).copied().unwrap_or(LADDER[ACTUAL_STEP]),
        ))
    }

    /// The size as the factor a length is multiplied by.
    #[must_use]
    pub fn factor(self) -> f32 {
        f32::from(LADDER.get(self.0).copied().unwrap_or(LADDER[ACTUAL_STEP])) / 1000.0
    }
}

impl Default for ContentScale {
    fn default() -> Self {
        ContentScale::ACTUAL
    }
}

/// The scale one step from the one shown: `step` thousandths bigger for `In`, smaller for `Out`.
fn stepped_target(viewport: Viewport, dir: ZoomDir, step: Permille) -> Permille {
    let shown = u64::from(viewport.shown.0.max(1));
    let step = u64::from(step.0.max(1001));
    let target = match dir {
        ZoomDir::In => shown * step / 1000,
        ZoomDir::Out => shown * 1000 / step,
    };
    Permille(u32::try_from(target).unwrap_or(u32::MAX))
}

/// The zoom one step from the scale shown: `step` is the factor in thousandths (1250 is x1.25).
/// A step out that reaches the fit scale is the fit zoom itself, so zooming out is a way back to
/// it; every other result is clamped to the zoom limits.
pub fn stepped(viewport: Viewport, dir: ZoomDir, step: Permille) -> Zoom {
    let target = stepped_target(viewport, dir, step);
    match dir {
        ZoomDir::Out if target <= viewport.fit => Zoom::Fit,
        ZoomDir::In | ZoomDir::Out => Zoom::scaled(target),
    }
}

/// The zoom one step from the scale shown, for content that may be shown smaller than it fits:
/// a step out goes on below the fit scale down to [`Viewport::floor`], and a step that crosses
/// the fit scale, either way, is the fit zoom itself, so fit is always a stop on the way.
pub fn stepped_past_fit(viewport: Viewport, dir: ZoomDir, step: Permille) -> Zoom {
    let shown = u64::from(viewport.shown.0.max(1));
    let target = stepped_target(viewport, dir, step);
    let crosses = match dir {
        ZoomDir::Out => u64::from(viewport.fit.0) < shown && target <= viewport.fit,
        ZoomDir::In => viewport.shown < viewport.fit && target >= viewport.fit,
    };
    if crosses {
        Zoom::Fit
    } else {
        Zoom::scaled(Permille(target.0.max(viewport.floor().0)))
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
    fn a_step_past_fit_goes_below_it_to_the_floor_and_crosses_fit_as_fit() {
        let at = |shown, fit| Viewport {
            shown: Permille(shown),
            fit: Permille(fit),
        };
        // name, viewport, direction, zoom
        let cases = [
            (
                "out from fit goes below it",
                at(1000, 1000),
                ZoomDir::Out,
                Zoom::Scale(Permille(800)),
            ),
            (
                "out that crosses fit is fit",
                at(1200, 1000),
                ZoomDir::Out,
                Zoom::Fit,
            ),
            (
                "out stops at a tenth",
                at(110, 1000),
                ZoomDir::Out,
                Zoom::Scale(Permille(100)),
            ),
            (
                "out stays at the floor",
                at(100, 1000),
                ZoomDir::Out,
                Zoom::Scale(Permille(100)),
            ),
            (
                "the floor of a large picture is a quarter of its fit",
                at(60, 200),
                ZoomDir::Out,
                Zoom::Scale(Permille(50)),
            ),
            (
                "in from below fit that reaches it is fit",
                at(900, 1000),
                ZoomDir::In,
                Zoom::Fit,
            ),
            (
                "in from below fit stays below",
                at(400, 1000),
                ZoomDir::In,
                Zoom::Scale(Permille(500)),
            ),
        ];
        for (name, viewport, dir, want) in cases {
            assert_eq!(stepped_past_fit(viewport, dir, STEP), want, "{name}");
        }
    }

    #[test]
    fn content_zoom_steps_along_its_ladder_and_stops_at_the_ends() {
        // name, steps taken (true is in), size after in thousandths
        const CASES: &[(&str, &[bool], u32)] = &[
            ("untouched", &[], 1000),
            ("in", &[true], 1250),
            ("out", &[false], 750),
            ("in twice", &[true, true], 1500),
            (
                "out to the floor and past it",
                &[false, false, false, false],
                500,
            ),
            ("in to the top and past it", &[true; 9], 3000),
            ("in and back out", &[true, true, false, false], 1000),
        ];
        for (name, steps, want) in CASES {
            let scale = steps.iter().fold(ContentScale::ACTUAL, |scale, in_| {
                scale.stepped(if *in_ { ZoomDir::In } else { ZoomDir::Out })
            });
            assert_eq!(scale.permille(), Permille(*want), "{name}");
        }
        assert!((ContentScale::ACTUAL.factor() - 1.0).abs() < f32::EPSILON);
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
