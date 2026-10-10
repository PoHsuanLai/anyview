//! The crop rectangle: where its grips are, how dragging one moves it, and what each aspect makes
//! of it. Pure arithmetic over whole pixels of the picture as it is shown (cut, mirrored and
//! turned, before any resize); a pointer comes in as a `DocPoint`, in 64ths of a pixel.

use anyview_core::{DocPoint, DocUnit, PixelLen, PixelRect, PixelSize};

/// The least a side of the rectangle can be, so that it can still be taken hold of.
const LEAST: i64 = 16;

/// Units in one pixel, as the arithmetic below counts them.
const PER_PIXEL: i64 = DocUnit::PER_PIXEL as i64;

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
    fn sides(self) -> (bool, bool, bool, bool) {
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

/// The shape a crop is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CropShape {
    /// Any shape.
    #[default]
    Free,
    /// The picture's own proportions.
    Original,
    /// Equal sides.
    Square,
    /// Four to three.
    FourThree,
    /// Sixteen to nine.
    SixteenNine,
}

impl CropShape {
    /// Every shape, in the order the control lists them.
    pub const ALL: [CropShape; 5] = [
        CropShape::Free,
        CropShape::Original,
        CropShape::Square,
        CropShape::FourThree,
        CropShape::SixteenNine,
    ];

    /// What the control calls it.
    pub fn label(self) -> &'static str {
        match self {
            CropShape::Free => "Free",
            CropShape::Original => "Original",
            CropShape::Square => "Square",
            CropShape::FourThree => "4:3",
            CropShape::SixteenNine => "16:9",
        }
    }
}

/// Whether a shape that has a long side lies down or stands up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CropLean {
    /// Wider than it is tall.
    #[default]
    Wide,
    /// Taller than it is wide.
    Tall,
}

impl CropLean {
    /// What the control calls it.
    pub fn label(self) -> &'static str {
        match self {
            CropLean::Wide => "Landscape",
            CropLean::Tall => "Portrait",
        }
    }
}

/// The proportions a crop is held to: a shape, lying down or standing up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CropAspect {
    /// The shape.
    pub shape: CropShape,
    /// Whether it is landscape or portrait. Free and Square have no long side to lean.
    pub lean: CropLean,
}

impl CropAspect {
    /// The width and the height the crop is held to, for a picture shown at `shown`; `None` when
    /// the shape is free.
    pub fn ratio(self, shown: PixelSize) -> Option<(u32, u32)> {
        let wide = match self.shape {
            CropShape::Free => return None,
            CropShape::Original => return Some((shown.width.0.max(1), shown.height.0.max(1))),
            CropShape::Square => return Some((1, 1)),
            CropShape::FourThree => (4, 3),
            CropShape::SixteenNine => (16, 9),
        };
        Some(match self.lean {
            CropLean::Wide => wide,
            CropLean::Tall => (wide.1, wide.0),
        })
    }
}

fn pixels(units: i64) -> i64 {
    (units + PER_PIXEL / 2).div_euclid(PER_PIXEL)
}

fn px(value: u32) -> i64 {
    i64::from(value)
}

/// `value` as pixels, held to what a `u32` holds.
fn to_u32(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

impl CropBox {
    /// The whole picture.
    pub fn whole(shown: PixelSize) -> CropBox {
        CropBox {
            left: 0,
            top: 0,
            width: shown.width.0,
            height: shown.height.0,
        }
    }

    /// The column just past the rectangle.
    pub fn right(self) -> u32 {
        self.left.saturating_add(self.width)
    }

    /// The row just past the rectangle.
    pub fn bottom(self) -> u32 {
        self.top.saturating_add(self.height)
    }

    /// The rectangle as a rectangle of pixels.
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
    pub fn thirds(self) -> ([u32; 2], [u32; 2]) {
        let at = |from: u32, extent: u32, third: u32| from + extent * third / 3;
        (
            [at(self.left, self.width, 1), at(self.left, self.width, 2)],
            [at(self.top, self.height, 1), at(self.top, self.height, 2)],
        )
    }

    /// The grip a press at `at` takes hold of, with `reach` the distance from an edge that still
    /// counts as on it; `None` when the press is farther than that outside the rectangle.
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

    /// This rectangle after `grip` was dragged by `by` (the pointer's move since the press, in
    /// 64ths of a pixel), kept inside a picture shown at `shown`. With a `ratio` the rectangle
    /// keeps its proportions as a corner or an edge moves: a corner grows from the opposite
    /// corner, an edge from the opposite edge, with the other side kept centred.
    pub fn dragged(
        self,
        grip: CropGrip,
        by: (i64, i64),
        shown: PixelSize,
        ratio: Option<(u32, u32)>,
    ) -> CropBox {
        let (dx, dy) = (pixels(by.0), pixels(by.1));
        let (width, height) = (px(shown.width.0), px(shown.height.0));
        if grip == CropGrip::Body {
            return CropBox {
                left: to_u32((px(self.left) + dx).clamp(0, (width - px(self.width)).max(0))),
                top: to_u32((px(self.top) + dy).clamp(0, (height - px(self.height)).max(0))),
                ..self
            };
        }
        let (west, east, north, south) = grip.sides();
        let least_x = LEAST.min(width);
        let least_y = LEAST.min(height);
        let (mut l, mut t) = (px(self.left), px(self.top));
        let (mut r, mut b) = (px(self.right()), px(self.bottom()));
        if west {
            l = (l + dx).clamp(0, r - least_x);
        }
        if east {
            r = (r + dx).clamp(l + least_x, width);
        }
        if north {
            t = (t + dy).clamp(0, b - least_y);
        }
        if south {
            b = (b + dy).clamp(t + least_y, height);
        }
        match ratio {
            None => CropBox {
                left: to_u32(l),
                top: to_u32(t),
                width: to_u32(r - l),
                height: to_u32(b - t),
            },
            Some(ratio) => self.held_to(grip, (l, t, r, b), shown, ratio),
        }
    }

    /// The rectangle `grip` makes of the free one `(l, t, r, b)`, held to `ratio`.
    fn held_to(
        self,
        grip: CropGrip,
        (l, t, r, b): (i64, i64, i64, i64),
        shown: PixelSize,
        (rw, rh): (u32, u32),
    ) -> CropBox {
        let (rw, rh) = (px(rw.max(1)), px(rh.max(1)));
        let (west, east, north, south) = grip.sides();
        let (width, height) = (px(shown.width.0), px(shown.height.0));
        let (mut w, mut h) = (r - l, b - t);
        let corner = (west || east) && (north || south);
        // Which way the rectangle grows decides how much room there is for it.
        let (room_w, room_h) = if corner {
            (
                if west {
                    px(self.right())
                } else {
                    width - px(self.left)
                },
                if north {
                    px(self.bottom())
                } else {
                    height - px(self.top)
                },
            )
        } else if west || east {
            (
                if west {
                    px(self.right())
                } else {
                    width - px(self.left)
                },
                height,
            )
        } else {
            (
                width,
                if north {
                    px(self.bottom())
                } else {
                    height - px(self.top)
                },
            )
        };
        // The side that was dragged leads; a corner follows whichever side asks for more.
        if corner {
            w = w.max((h * rw + rh / 2) / rh);
        } else if north || south {
            w = (h * rw + rh / 2) / rh;
        }
        // Held inside the room, in both directions at once.
        w = w.min(room_w).min((room_h * rw) / rh).max(1);
        h = ((w * rh + rw / 2) / rw).max(1);
        if h > room_h {
            h = room_h;
            w = ((h * rw + rh / 2) / rh).max(1);
        }
        let (left, top) = if corner {
            (
                if west {
                    px(self.right()) - w
                } else {
                    px(self.left)
                },
                if north {
                    px(self.bottom()) - h
                } else {
                    px(self.top)
                },
            )
        } else if west || east {
            let middle = px(self.top) + px(self.height) / 2;
            (
                if west {
                    px(self.right()) - w
                } else {
                    px(self.left)
                },
                (middle - h / 2).clamp(0, (height - h).max(0)),
            )
        } else {
            let middle = px(self.left) + px(self.width) / 2;
            (
                (middle - w / 2).clamp(0, (width - w).max(0)),
                if north {
                    px(self.bottom()) - h
                } else {
                    px(self.top)
                },
            )
        };
        CropBox {
            left: to_u32(left.clamp(0, (width - w).max(0))),
            top: to_u32(top.clamp(0, (height - h).max(0))),
            width: to_u32(w),
            height: to_u32(h),
        }
    }

    /// The largest rectangle of `ratio` that fits inside this one, about its middle.
    pub fn shaped(self, (rw, rh): (u32, u32)) -> CropBox {
        let (rw, rh) = (px(rw.max(1)), px(rh.max(1)));
        let (width, height) = (px(self.width), px(self.height));
        let (w, h) = if width * rh >= height * rw {
            (((height * rw + rh / 2) / rh).max(1), height.max(1))
        } else {
            (width.max(1), ((width * rh + rw / 2) / rw).max(1))
        };
        let (w, h) = (w.min(width.max(1)), h.min(height.max(1)));
        CropBox {
            left: to_u32(px(self.left) + (width - w) / 2),
            top: to_u32(px(self.top) + (height - h) / 2),
            width: to_u32(w),
            height: to_u32(h),
        }
    }
}
