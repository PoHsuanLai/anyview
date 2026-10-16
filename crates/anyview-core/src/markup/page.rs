//! Where on a page: a point and a rectangle, in thousandths of the page as displayed.

use crate::units::Permille;

/// A point on a page as displayed: each coordinate a thousandth of the page's width (`x`) or
/// height (`y`), from the top left. Always on the page: a coordinate past the whole is held at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PagePoint {
    x: Permille,
    y: Permille,
}

/// `value` held on the page.
const fn held(value: Permille) -> Permille {
    if value.0 > 1000 {
        Permille::WHOLE
    } else {
        value
    }
}

/// `value` moved by `by`, held on the page.
pub(super) fn shifted(value: Permille, by: i32) -> Permille {
    let moved = i64::from(value.0) + i64::from(by);
    Permille(u32::try_from(moved.clamp(0, 1000)).unwrap_or(0))
}

impl PagePoint {
    /// The point `x` across and `y` down, each held on the page.
    #[must_use]
    pub const fn new(x: Permille, y: Permille) -> PagePoint {
        PagePoint {
            x: held(x),
            y: held(y),
        }
    }

    /// Across, 0 at the left edge and 1000 at the right.
    #[must_use]
    pub const fn x(self) -> Permille {
        self.x
    }

    /// Down, 0 at the top edge and 1000 at the bottom.
    #[must_use]
    pub const fn y(self) -> Permille {
        self.y
    }

    /// The point `x` and `y` thousandths in, held on the page.
    #[must_use]
    pub fn at(x: u32, y: u32) -> PagePoint {
        PagePoint {
            x: Permille(x.min(1000)),
            y: Permille(y.min(1000)),
        }
    }

    /// This point moved by `dx` across and `dy` down, held on the page.
    #[must_use]
    pub fn moved(self, dx: i32, dy: i32) -> PagePoint {
        PagePoint {
            x: shifted(self.x, dx),
            y: shifted(self.y, dy),
        }
    }
}

/// A rectangle on a page as displayed: each edge a thousandth of the page's width (left, right) or
/// height (top, bottom), from the top left, so 0 to 1000 spans the page. Always on the page and
/// never reversed: the left edge is not past the right, nor the top past the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageRect {
    left: Permille,
    top: Permille,
    right: Permille,
    bottom: Permille,
}

impl PageRect {
    /// The rectangle with these edges, each held on the page, and a pair given the wrong way
    /// round (left past right, top past bottom) swapped.
    #[must_use]
    pub const fn new(left: Permille, top: Permille, right: Permille, bottom: Permille) -> PageRect {
        let (left, right) = (held(left), held(right));
        let (top, bottom) = (held(top), held(bottom));
        let (left, right) = if left.0 > right.0 {
            (right, left)
        } else {
            (left, right)
        };
        let (top, bottom) = if top.0 > bottom.0 {
            (bottom, top)
        } else {
            (top, bottom)
        };
        PageRect {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The left edge.
    #[must_use]
    pub const fn left(self) -> Permille {
        self.left
    }

    /// The top edge.
    #[must_use]
    pub const fn top(self) -> Permille {
        self.top
    }

    /// The right edge.
    #[must_use]
    pub const fn right(self) -> Permille {
        self.right
    }

    /// The bottom edge.
    #[must_use]
    pub const fn bottom(self) -> Permille {
        self.bottom
    }

    /// The rectangle with these two opposite corners, whichever way round they are given.
    #[must_use]
    pub fn spanning(a: PagePoint, b: PagePoint) -> PageRect {
        PageRect {
            left: Permille(a.x.0.min(b.x.0)),
            top: Permille(a.y.0.min(b.y.0)),
            right: Permille(a.x.0.max(b.x.0)),
            bottom: Permille(a.y.0.max(b.y.0)),
        }
    }

    /// How wide it is, in thousandths of the page.
    #[must_use]
    pub fn width(self) -> u32 {
        self.right.0.saturating_sub(self.left.0)
    }

    /// How high it is, in thousandths of the page.
    #[must_use]
    pub fn height(self) -> u32 {
        self.bottom.0.saturating_sub(self.top.0)
    }

    /// Whether `at` is inside it or within `reach` thousandths of it.
    #[must_use]
    pub fn near(self, at: PagePoint, reach: u32) -> bool {
        at.x.0.saturating_add(reach) >= self.left.0
            && at.x.0 <= self.right.0.saturating_add(reach)
            && at.y.0.saturating_add(reach) >= self.top.0
            && at.y.0 <= self.bottom.0.saturating_add(reach)
    }

    /// Whether the two overlap.
    #[must_use]
    pub fn meets(self, other: PageRect) -> bool {
        self.left.0 < other.right.0
            && other.left.0 < self.right.0
            && self.top.0 < other.bottom.0
            && other.top.0 < self.bottom.0
    }

    /// The smallest rectangle holding both.
    #[must_use]
    pub fn joined(self, other: PageRect) -> PageRect {
        PageRect {
            left: Permille(self.left.0.min(other.left.0)),
            top: Permille(self.top.0.min(other.top.0)),
            right: Permille(self.right.0.max(other.right.0)),
            bottom: Permille(self.bottom.0.max(other.bottom.0)),
        }
    }

    /// This rectangle moved by `dx` across and `dy` down, as far as the page lets it go with its
    /// size kept.
    #[must_use]
    pub fn moved(self, dx: i32, dy: i32) -> PageRect {
        let room = |edge: u32, far: u32, by: i32| {
            let low = -i32::try_from(edge).unwrap_or(0);
            let high = i32::try_from(1000u32.saturating_sub(far)).unwrap_or(0);
            by.clamp(low, high)
        };
        let (dx, dy) = (
            room(self.left.0, self.right.0, dx),
            room(self.top.0, self.bottom.0, dy),
        );
        PageRect {
            left: shifted(self.left, dx),
            top: shifted(self.top, dy),
            right: shifted(self.right, dx),
            bottom: shifted(self.bottom, dy),
        }
    }
}
