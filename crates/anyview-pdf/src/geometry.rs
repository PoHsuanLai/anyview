//! Sizes and rectangles in the page's own space, as integers: a page's size in thousandths of a
//! point, and a rectangle on a page as thousandths of the page's displayed width and height, so a
//! mark can be drawn at any zoom without asking the document again.

use anyview_core::Permille;
use pdfrum::{Rect, Rotation};

/// A length in thousandths of a point (1/72 inch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MilliPoints(pub u32);

impl MilliPoints {
    /// The length of `points`, rounded, and never zero (a page has some size).
    pub fn from_points(points: f64) -> MilliPoints {
        let milli = (points * 1000.0).round().clamp(1.0, f64::from(u32::MAX));
        MilliPoints(milli as u32)
    }
}

/// A page as displayed: its crop box after the page's own rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageSize {
    /// Displayed width.
    pub width: MilliPoints,
    /// Displayed height.
    pub height: MilliPoints,
}

impl PageSize {
    /// US Letter, standing in for a page that would not load so the layout keeps its place.
    pub const LETTER: PageSize = PageSize {
        width: MilliPoints(612_000),
        height: MilliPoints(792_000),
    };
}

/// A rectangle on a displayed page: each edge a thousandth of the page's displayed width (left,
/// right) or height (top, bottom), measured from the top left, so 0 to 1000 spans the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageRect {
    /// The left edge.
    pub left: Permille,
    /// The top edge.
    pub top: Permille,
    /// The right edge.
    pub right: Permille,
    /// The bottom edge.
    pub bottom: Permille,
}

/// `rect` (PDF user space: y up, the crop box's corner as origin) as it lies on the page as
/// displayed, which turns clockwise by `rotation`.
pub fn displayed(rect: Rect, crop: Rect, rotation: Rotation) -> PageRect {
    let (width, height) = (
        crop.width().max(f64::EPSILON),
        crop.height().max(f64::EPSILON),
    );
    // The unrotated page, y down, each axis 0 to 1.
    let upright = |x: f64, y: f64| ((x - crop.x0) / width, (crop.y1 - y) / height);
    let turned = |(u, v): (f64, f64)| match rotation {
        Rotation::None => (u, v),
        Rotation::Quarter => (1.0 - v, u),
        Rotation::Half => (1.0 - u, 1.0 - v),
        Rotation::ThreeQuarter => (v, 1.0 - u),
    };
    let (ax, ay) = turned(upright(rect.x0, rect.y0));
    let (bx, by) = turned(upright(rect.x1, rect.y1));
    let thousandths = |fraction: f64| Permille((fraction.clamp(0.0, 1.0) * 1000.0).round() as u32);
    PageRect {
        left: thousandths(ax.min(bx)),
        top: thousandths(ay.min(by)),
        right: thousandths(ax.max(bx)),
        bottom: thousandths(ay.max(by)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
        PageRect {
            left: Permille(left),
            top: Permille(top),
            right: Permille(right),
            bottom: Permille(bottom),
        }
    }

    #[test]
    fn a_rectangle_lands_where_the_page_is_turned_to() {
        // A 200 x 100 page; the box is the left tenth of the width, the top fifth of the height
        // (PDF y is up, so its y runs 80 to 100).
        let crop = Rect::new(0.0, 0.0, 200.0, 100.0);
        let probe = Rect::new(0.0, 80.0, 20.0, 100.0);
        const CASES: &[(&str, Rotation, PageRect)] = &[
            ("upright", Rotation::None, rect(0, 0, 100, 200)),
            (
                "quarter turn clockwise",
                Rotation::Quarter,
                rect(800, 0, 1000, 100),
            ),
            ("half turn", Rotation::Half, rect(900, 800, 1000, 1000)),
            (
                "three quarters",
                Rotation::ThreeQuarter,
                rect(0, 900, 200, 1000),
            ),
        ];
        for (name, rotation, want) in CASES {
            assert_eq!(displayed(probe, crop, *rotation), *want, "{name}");
        }
    }

    #[test]
    fn the_crop_box_origin_and_a_reversed_rectangle_are_honoured() {
        let crop = Rect::new(100.0, 100.0, 300.0, 200.0);
        let reversed = Rect::new(120.0, 200.0, 100.0, 180.0);
        assert_eq!(
            displayed(reversed, crop, Rotation::None),
            rect(0, 0, 100, 200)
        );
    }

    #[test]
    fn a_rectangle_off_the_page_is_cut_to_it() {
        let crop = Rect::new(0.0, 0.0, 100.0, 100.0);
        let off = Rect::new(-50.0, -50.0, 150.0, 150.0);
        assert_eq!(displayed(off, crop, Rotation::None), rect(0, 0, 1000, 1000));
    }

    #[test]
    fn lengths_round_to_thousandths_and_never_reach_zero() {
        assert_eq!(MilliPoints::from_points(595.276), MilliPoints(595_276));
        assert_eq!(MilliPoints::from_points(0.0), MilliPoints(1));
        assert_eq!(MilliPoints::from_points(-3.0), MilliPoints(1));
    }
}
