//! Which texels of the picture show and where they are drawn in the room.

use super::scale::{texels, turned};
use crate::Area;
use anyview_core::{DocPoint, Permille, PixelSize, QuarterTurn, Reflection};

/// Where the picture is drawn, in logical pixels of the room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    /// The texels to show: the upright texture's own, whole texels.
    pub source: (u32, u32, u32, u32),
    /// The box of the shown part in the room: left, top, width, height.
    pub shown: (f64, f64, f64, f64),
    /// The turn the texture's box is drawn at.
    pub turn: QuarterTurn,
}

/// The shown part of the picture and where it goes, or `None` when none of it is in view.
pub(crate) fn place(
    size: PixelSize,
    turn: QuarterTurn,
    scale: Permille,
    centre: DocPoint,
    area: Area,
) -> Option<Placed> {
    let (wr, hr) = turned(size, turn);
    let per_texel = f64::from(scale.0.max(1)) / 1000.0 / f64::from(area.scale.max(f32::EPSILON));
    let (aw, ah) = (f64::from(area.size.width.0), f64::from(area.size.height.0));
    let (cx, cy) = (texels(centre.x), texels(centre.y));
    let (left, top) = (cx - aw / (2.0 * per_texel), cy - ah / (2.0 * per_texel));
    let (right, bottom) = (cx + aw / (2.0 * per_texel), cy + ah / (2.0 * per_texel));
    let (x0, y0) = (left.max(0.0).floor(), top.max(0.0).floor());
    let (x1, y1) = (right.min(wr).ceil(), bottom.min(hr).ceil());
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let shown = (
        (x0 - left) * per_texel,
        (y0 - top) * per_texel,
        (x1 - x0) * per_texel,
        (y1 - y0) * per_texel,
    );
    Some(Placed {
        source: upright((x0, y0, x1 - x0, y1 - y0), size, turn),
        shown,
        turn,
    })
}

/// A rectangle (x, y, width, height) of the shown picture as one of the upright texture's.
fn upright(
    (x, y, w, h): (f64, f64, f64, f64),
    size: PixelSize,
    turn: QuarterTurn,
) -> (u32, u32, u32, u32) {
    let (width, height) = (f64::from(size.width.0), f64::from(size.height.0));
    let (ux, uy, uw, uh) = match turn {
        QuarterTurn::None => (x, y, w, h),
        // Shown (x, y) is the upright (y, height - x): a quarter turn clockwise.
        QuarterTurn::Quarter => (y, height - x - w, h, w),
        QuarterTurn::Half => (width - x - w, height - y - h, w, h),
        QuarterTurn::ThreeQuarter => (width - y - h, x, h, w),
    };
    // Whole texels inside the texture, so a rounding error never asks for one outside.
    let clip = |value: f64, extent: f64| value.round().clamp(0.0, extent) as u32;
    (
        clip(ux, width),
        clip(uy, height),
        clip(uw, width),
        clip(uh, height),
    )
}

/// `source`, texels of the picture at its own size, as the texels of a texture that holds the same
/// picture at the size `held` (a first frame, smaller than the picture): the same part of it, to
/// whole texels, never empty and never outside the texture.
pub(crate) fn held_source(
    (x, y, w, h): (u32, u32, u32, u32),
    size: PixelSize,
    held: PixelSize,
) -> (u32, u32, u32, u32) {
    let along = |from: u32, len: u32, of: u32, to: u32| {
        let of = u64::from(of.max(1));
        let first = (u64::from(from) * u64::from(to) / of).min(u64::from(to.saturating_sub(1)));
        let end = ((u64::from(from) + u64::from(len)) * u64::from(to))
            .div_ceil(of)
            .clamp(first + 1, u64::from(to.max(1)));
        // Both are at most `to`, which is a u32.
        (first as u32, (end - first) as u32)
    };
    let (hx, hw) = along(x, w, size.width.0, held.width.0);
    let (hy, hh) = along(y, h, size.height.0, held.height.0);
    (hx, hy, hw, hh)
}

/// A rectangle (x, y, width, height) of the picture's kept part, as laid out upright, as a
/// rectangle of the file's own picture: the mirror undone (the picture is mirrored before it is
/// turned, so this is within the part) and the part's place in the picture added.
pub(crate) fn in_file(
    (x, y, w, h): (u32, u32, u32, u32),
    kept: PixelSize,
    reflection: Reflection,
    (left, top): (u32, u32),
) -> (u32, u32, u32, u32) {
    let x = match reflection {
        Reflection::Kept => x,
        Reflection::Mirrored => kept.width.0.saturating_sub(x.saturating_add(w)),
    };
    (left.saturating_add(x), top.saturating_add(y), w, h)
}
