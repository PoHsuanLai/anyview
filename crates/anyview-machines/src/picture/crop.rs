//! The crop rectangle and its grips: where they are on the shown picture. Pure arithmetic over
//! whole pixels of the picture as it is shown (cut, mirrored and turned, before any resize); a
//! pointer comes in as a `DocPoint`, in 64ths of a pixel.

use anyview_core::{DocPoint, DocUnit, PixelLen, PixelRect, PixelSize};

/// Units in one pixel, as the arithmetic below counts them.
pub(super) const PER_PIXEL: i64 = DocUnit::PER_PIXEL as i64;

/// A rectangle of the shown picture, in whole pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CropBox {
    /// Columns from the picture's left edge.
    pub left: u32,
    /// Rows from the picture's top edge.
    pub top: u32,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

/// Where on the rectangle a press took hold: a corner or an edge resizes it, the inside moves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CropGrip {
    /// The top left corner.
    NorthWest,
    /// The top edge.
    North,
    /// The top right corner.
    NorthEast,
    /// The right edge.
    East,
    /// The bottom right corner.
    SouthEast,
    /// The bottom edge.
    South,
    /// The bottom left corner.
    SouthWest,
    /// The left edge.
    West,
    /// The inside.
    Body,
}

impl CropGrip {
    /// The eight grips that resize, clockwise from the top left.
    pub const HANDLES: [CropGrip; 8] = [
        CropGrip::NorthWest,
        CropGrip::North,
        CropGrip::NorthEast,
        CropGrip::East,
        CropGrip::SouthEast,
        CropGrip::South,
        CropGrip::SouthWest,
        CropGrip::West,
    ];

    /// Which of the four sides this grip moves: west, east, north, south.
    pub(super) fn sides(self) -> (bool, bool, bool, bool) {
        match self {
            CropGrip::NorthWest => (true, false, true, false),
            CropGrip::North => (false, false, true, false),
            CropGrip::NorthEast => (false, true, true, false),
            CropGrip::East => (false, true, false, false),
            CropGrip::SouthEast => (false, true, false, true),
            CropGrip::South => (false, false, false, true),
            CropGrip::SouthWest => (true, false, false, true),
            CropGrip::West => (true, false, false, false),
            CropGrip::Body => (false, false, false, false),
        }
    }

    /// The word the grip is named by in markup.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            CropGrip::NorthWest => "north-west",
            CropGrip::North => "north",
            CropGrip::NorthEast => "north-east",
            CropGrip::East => "east",
            CropGrip::SouthEast => "south-east",
            CropGrip::South => "south",
            CropGrip::SouthWest => "south-west",
            CropGrip::West => "west",
            CropGrip::Body => "body",
        }
    }
}

pub(super) fn px(value: u32) -> i64 {
    i64::from(value)
}

impl CropBox {
    /// The whole picture.
    #[must_use]
    pub fn whole(shown: PixelSize) -> CropBox {
        CropBox {
            left: 0,
            top: 0,
            width: shown.width.0,
            height: shown.height.0,
        }
    }

    /// The column just past the rectangle.
    #[must_use]
    pub fn right(self) -> u32 {
        self.left.saturating_add(self.width)
    }

    /// The row just past the rectangle.
    #[must_use]
    pub fn bottom(self) -> u32 {
        self.top.saturating_add(self.height)
    }

    /// The rectangle as a rectangle of pixels.
    #[must_use]
    pub fn rect(self) -> PixelRect {
        PixelRect {
            left: PixelLen(self.left),
            top: PixelLen(self.top),
            size: PixelSize {
                width: PixelLen(self.width),
                height: PixelLen(self.height),
            },
        }
    }

    /// The columns of the two vertical guides that cut the rectangle in thirds, and the rows of
    /// the two horizontal ones.
    #[must_use]
    pub fn thirds(self) -> ([u32; 2], [u32; 2]) {
        let at = |from: u32, extent: u32, third: u32| from + extent * third / 3;
        (
            [at(self.left, self.width, 1), at(self.left, self.width, 2)],
            [at(self.top, self.height, 1), at(self.top, self.height, 2)],
        )
    }

    /// The grip a press at `at` takes hold of, with `reach` the distance from an edge that still
    /// counts as on it; `None` when the press is farther than that outside the rectangle.
    #[must_use]
    pub fn grip_at(self, at: DocPoint, reach: DocUnit) -> Option<CropGrip> {
        let (west, east) = (px(self.left) * PER_PIXEL, px(self.right()) * PER_PIXEL);
        let (north, south) = (px(self.top) * PER_PIXEL, px(self.bottom()) * PER_PIXEL);
        let (x, y) = (i64::from(at.x.0), i64::from(at.y.0));
        let reach = i64::from(reach.0).max(0);
        if x < west - reach || x > east + reach || y < north - reach || y > south + reach {
            return None;
        }
        let near = |value: i64, edge: i64| (value - edge).abs() <= reach;
        let closer =
            |value: i64, edge: i64, other: i64| (value - edge).abs() <= (value - other).abs();
        let left = near(x, west) && (!near(x, east) || closer(x, west, east));
        let right = near(x, east) && !left;
        let top = near(y, north) && (!near(y, south) || closer(y, north, south));
        let bottom = near(y, south) && !top;
        Some(if left && top {
            CropGrip::NorthWest
        } else if right && top {
            CropGrip::NorthEast
        } else if right && bottom {
            CropGrip::SouthEast
        } else if left && bottom {
            CropGrip::SouthWest
        } else if top {
            CropGrip::North
        } else if right {
            CropGrip::East
        } else if bottom {
            CropGrip::South
        } else if left {
            CropGrip::West
        } else {
            CropGrip::Body
        })
    }
}
