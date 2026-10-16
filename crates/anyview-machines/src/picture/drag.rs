//! Dragging a grip of the crop rectangle, and holding it to a ratio.

use super::crop::{CropBox, CropGrip, PER_PIXEL, px};
use anyview_core::PixelSize;

/// The least a side of the rectangle can be, so that it can still be taken hold of.
const LEAST: i64 = 16;

fn pixels(units: i64) -> i64 {
    (units + PER_PIXEL / 2).div_euclid(PER_PIXEL)
}

/// `value` as pixels, held to what a `u32` holds.
fn to_u32(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

impl CropBox {
    /// This rectangle after `grip` was dragged by `by` (the pointer's move since the press, in
    /// 64ths of a pixel), kept inside a picture shown at `shown`. With a `ratio` the rectangle
    /// keeps its proportions as a corner or an edge moves: a corner grows from the opposite
    /// corner, an edge from the opposite edge, with the other side kept centred.
    #[must_use]
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
        // A rectangle can be smaller than `LEAST` (a held shape makes one), so each bound is kept
        // inside the picture before it is clamped to.
        let (west, east, north, south) = grip.sides();
        let least_x = LEAST.min(width);
        let least_y = LEAST.min(height);
        let (mut l, mut t) = (px(self.left), px(self.top));
        let (mut r, mut b) = (px(self.right()), px(self.bottom()));
        if west {
            l = (l + dx).clamp(0, (r - least_x).max(0));
        }
        if east {
            r = (r + dx).clamp((l + least_x).min(width), width);
        }
        if north {
            t = (t + dy).clamp(0, (b - least_y).max(0));
        }
        if south {
            b = (b + dy).clamp((t + least_y).min(height), height);
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
    #[must_use]
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
