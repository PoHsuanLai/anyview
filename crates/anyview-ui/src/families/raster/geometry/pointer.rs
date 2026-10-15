//! A pointer in the room as a point of the picture, and how far a drag pans it.

use super::scale::{texels, turned, units};
use crate::Area;
use anyview_core::{DocPoint, DocUnit, Permille, PixelSize, QuarterTurn};

/// Logical pixels of the room per texel at `scale` device pixels per texel.
fn logical_per_texel(scale: Permille, area: Area) -> f64 {
    f64::from(scale.0.max(1)) / 1000.0 / f64::from(area.scale.max(f32::EPSILON))
}

/// Where the picture point (`x`, `y`, in texels as shown) is in the room, in logical pixels from the
/// room's corner: what `point_under` undoes.
pub(crate) fn room_point(
    centre: DocPoint,
    scale: Permille,
    area: Area,
    (x, y): (f64, f64),
) -> (f64, f64) {
    let per_texel = logical_per_texel(scale, area);
    (
        f64::from(area.size.width.0) / 2.0 + (x - texels(centre.x)) * per_texel,
        f64::from(area.size.height.0) / 2.0 + (y - texels(centre.y)) * per_texel,
    )
}

/// A length of `logical` pixels of the room as a length in the picture, in 1/64 texel.
pub(crate) fn reach_in_picture(scale: Permille, area: Area, logical: f32) -> DocUnit {
    units(f64::from(logical) / logical_per_texel(scale, area))
}

/// The picture point (in 1/64 texel, as shown) under a pointer at (`x`, `y`) in the room.
pub(crate) fn point_under(
    centre: DocPoint,
    scale: Permille,
    area: Area,
    (x, y): (f32, f32),
) -> DocPoint {
    let per_texel = f64::from(scale.0.max(1)) / 1000.0 / f64::from(area.scale.max(f32::EPSILON));
    let from_middle = (
        f64::from(x) - f64::from(area.size.width.0) / 2.0,
        f64::from(y) - f64::from(area.size.height.0) / 2.0,
    );
    DocPoint {
        x: units(texels(centre.x) + from_middle.0 / per_texel),
        y: units(texels(centre.y) + from_middle.1 / per_texel),
    }
}

/// How far the picture moves for a pointer that moved (`dx`, `dy`) logical pixels, in 1/64 texel.
pub(crate) fn pointer_delta(scale: Permille, area: Area, (dx, dy): (f32, f32)) -> DocPoint {
    let per_texel = f64::from(scale.0.max(1)) / 1000.0 / f64::from(area.scale.max(f32::EPSILON));
    DocPoint {
        x: units(f64::from(dx) / per_texel),
        y: units(f64::from(dy) / per_texel),
    }
}

/// The part of a pan of `by` (the pointer's move, in 1/64 texel) that the picture can follow from
/// `centre`: the room never shows past the picture's edge, and a picture narrower or shorter than
/// the room on an axis moves only until its edge meets the room's, so it is never carried out of
/// view.
pub(crate) fn limited_pan(
    size: PixelSize,
    turn: QuarterTurn,
    scale: Permille,
    centre: DocPoint,
    area: Area,
    by: DocPoint,
) -> DocPoint {
    let (wr, hr) = turned(size, turn);
    let per_texel = f64::from(scale.0.max(1)) / 1000.0 / f64::from(area.scale.max(f32::EPSILON));
    let half = (
        f64::from(area.size.width.0) / (2.0 * per_texel),
        f64::from(area.size.height.0) / (2.0 * per_texel),
    );
    let along = |from: DocUnit, by: DocUnit, extent: f64, half: f64| {
        let wanted = texels(from) - texels(by);
        // The centre where the picture's start meets the room's start, and where its end meets the
        // room's end; the pan stays between them, whichever way round they fall.
        let (near, far) = (half, extent - half);
        let reach = wanted.clamp(near.min(far), near.max(far));
        DocUnit(from.0.saturating_sub(units(reach).0))
    };
    DocPoint {
        x: along(centre.x, by.x, wr, half.0),
        y: along(centre.y, by.y, hr, half.1),
    }
}
