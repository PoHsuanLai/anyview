//! Where a picture lands in the room it has: which texels show and where they are drawn. Pure
//! arithmetic over the stage machine's state, tested as tables. Content points and scales are the
//! machine's integers (`DocPoint` in 1/64 texel, `Permille`); a float appears inside the
//! arithmetic only.
//!
//! Everything the machine knows is in the picture *as shown*, that is, after its turn; the texture
//! is always the upright picture. So the layout works out the visible part in shown coordinates
//! and maps it back to the texture's own with the inverse of the turn, and the view draws the
//! texture's box rotated about its middle.

use crate::{Animation, Area, FrameIndex, RasterStage};
use anyview_core::{DocPoint, DocUnit, Permille, PixelSize, QuarterTurn, Reflection, Zoom};

/// The picture's width and height in texels as shown: a quarter turn swaps them.
pub(crate) fn turned(size: PixelSize, turn: QuarterTurn) -> (f64, f64) {
    let (w, h) = (f64::from(size.width.0), f64::from(size.height.0));
    match turn {
        QuarterTurn::None | QuarterTurn::Half => (w, h),
        QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => (h, w),
    }
}

/// The turn the stage is at.
pub(crate) fn turn_of(stage: &RasterStage) -> QuarterTurn {
    match stage {
        RasterStage::Fitted { turn, .. }
        | RasterStage::Zoomed { turn, .. }
        | RasterStage::Panning { turn, .. } => *turn,
    }
}

/// Whether the picture moves, and which frame it is on.
pub(crate) fn animation_of(stage: &RasterStage) -> Animation {
    match stage {
        RasterStage::Fitted { anim, .. }
        | RasterStage::Zoomed { anim, .. }
        | RasterStage::Panning { anim, .. } => *anim,
    }
}

/// The frame on screen: the one the animation is on, or the first.
pub(crate) fn frame_of(stage: &RasterStage) -> FrameIndex {
    match animation_of(stage) {
        Animation::Still => FrameIndex(0),
        Animation::Playing { frame, .. }
        | Animation::Paused { frame, .. }
        | Animation::Ended { frame, .. } => frame,
    }
}

/// Device pixels per texel at which the whole picture fits `area`, never above actual size: a
/// small picture is shown at its own size, not stretched.
pub(crate) fn fit(size: PixelSize, turn: QuarterTurn, area: Area) -> Permille {
    let (w, h) = turned(size, turn);
    let device = (
        f64::from(area.size.width.0) * f64::from(area.scale),
        f64::from(area.size.height.0) * f64::from(area.scale),
    );
    let scale = (device.0 / w.max(1.0)).min(device.1 / h.max(1.0)).min(1.0);
    Permille(per_mille(scale).max(Zoom::MIN_SCALE.0))
}

/// The scale the stage draws at, in device pixels per texel.
pub(crate) fn scale_of(stage: &RasterStage, fit: Permille) -> Permille {
    match stage {
        RasterStage::Fitted { .. } => fit,
        RasterStage::Zoomed { zoom, .. } | RasterStage::Panning { zoom, .. } => match zoom {
            Zoom::Fit | Zoom::Fill => fit,
            Zoom::Actual => Permille::WHOLE,
            Zoom::Scale(scale) => *scale,
        },
    }
}

/// The picture point in the middle of the room, as shown: for a picture of `size` turned by
/// `turn`, the middle of it when it is fitted.
pub(crate) fn centre_of(stage: &RasterStage, size: PixelSize, turn: QuarterTurn) -> DocPoint {
    match stage {
        RasterStage::Zoomed { centre, .. } | RasterStage::Panning { centre, .. } => *centre,
        RasterStage::Fitted { .. } => {
            let (w, h) = turned(size, turn);
            DocPoint {
                x: units(w / 2.0),
                y: units(h / 2.0),
            }
        }
    }
}

fn per_mille(scale: f64) -> u32 {
    // A scale is positive and below 2^32 / 1000 whatever the picture, so the cast cannot wrap.
    (scale * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
}

fn units(texels: f64) -> DocUnit {
    DocUnit((texels * f64::from(DocUnit::PER_PIXEL)).round() as i32)
}

fn texels(point: DocUnit) -> f64 {
    f64::from(point.0) / f64::from(DocUnit::PER_PIXEL)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::PixelLen;
    use ds::prelude::{Point, Px, Size};

    const SIZE: PixelSize = PixelSize {
        width: PixelLen(400),
        height: PixelLen(200),
    };

    fn area(w: f32, h: f32, scale: f32) -> Area {
        Area {
            origin: Point {
                x: Px(0.0),
                y: Px(0.0),
            },
            size: Size {
                width: Px(w),
                height: Px(h),
            },
            scale,
        }
    }

    fn point(x: f64, y: f64) -> DocPoint {
        DocPoint {
            x: units(x),
            y: units(y),
        }
    }

    #[test]
    fn the_fit_scale_holds_the_whole_picture_and_never_enlarges() {
        // name, turn, room (w, h, device scale), scale in thousandths
        type FitCase = (&'static str, QuarterTurn, (f32, f32, f32), u32);
        const CASES: &[FitCase] = &[
            (
                "wide in a square room",
                QuarterTurn::None,
                (200.0, 200.0, 1.0),
                500,
            ),
            (
                "turned, it is tall: a tall room holds it",
                QuarterTurn::Quarter,
                (200.0, 400.0, 1.0),
                1000,
            ),
            (
                "a roomy window shows actual size",
                QuarterTurn::None,
                (900.0, 900.0, 1.0),
                1000,
            ),
            (
                "the device scale counts",
                QuarterTurn::None,
                (200.0, 200.0, 2.0),
                1000,
            ),
            (
                "the device scale counts down",
                QuarterTurn::None,
                (100.0, 100.0, 2.0),
                500,
            ),
        ];
        for (name, turn, (w, h, scale), want) in CASES {
            assert_eq!(fit(SIZE, *turn, area(*w, *h, *scale)).0, *want, "{name}");
        }
    }

    #[test]
    fn a_fitted_picture_is_drawn_whole_and_centred() {
        let room = area(1000.0, 600.0, 1.0);
        let placed = place(
            SIZE,
            QuarterTurn::None,
            Permille(1000),
            point(200.0, 100.0),
            room,
        )
        .expect("in view");
        assert_eq!(placed.source, (0, 0, 400, 200));
        assert_eq!(placed.shown, (300.0, 200.0, 400.0, 200.0));
    }

    #[test]
    fn a_zoomed_picture_shows_the_texels_around_the_centre() {
        // 4x of a 400 x 200 picture in a 400 x 200 room, centred on (100, 50): the room holds
        // 100 x 50 texels, from (50, 25).
        let room = area(400.0, 200.0, 1.0);
        let placed = place(
            SIZE,
            QuarterTurn::None,
            Permille(4000),
            point(100.0, 50.0),
            room,
        )
        .expect("in view");
        assert_eq!(placed.source, (50, 25, 100, 50));
        assert_eq!(placed.shown, (0.0, 0.0, 400.0, 200.0));
    }

    #[test]
    fn a_picture_dragged_off_the_room_is_none() {
        let room = area(100.0, 100.0, 1.0);
        assert_eq!(
            place(
                SIZE,
                QuarterTurn::None,
                Permille(1000),
                point(5000.0, 100.0),
                room
            ),
            None
        );
    }

    #[test]
    fn a_turn_maps_the_shown_texels_back_to_the_upright_ones() {
        // The whole 400 x 200 picture, turned, as shown it is 200 wide and 400 tall; the shown
        // top-left 50 x 50 corner is a different corner of the upright texture for each turn.
        let room = area(50.0, 50.0, 1.0);
        // name, turn, the upright corner (x, y, w, h)
        type TurnCase = (&'static str, QuarterTurn, (u32, u32, u32, u32));
        const CASES: &[TurnCase] = &[
            ("none", QuarterTurn::None, (0, 0, 50, 50)),
            (
                "quarter: shown top-left is upright bottom-left",
                QuarterTurn::Quarter,
                (0, 150, 50, 50),
            ),
            (
                "half: shown top-left is upright bottom-right",
                QuarterTurn::Half,
                (350, 150, 50, 50),
            ),
            (
                "three quarters: shown top-left is upright top-right",
                QuarterTurn::ThreeQuarter,
                (350, 0, 50, 50),
            ),
        ];
        for (name, turn, want) in CASES {
            let placed = place(SIZE, *turn, Permille(1000), point(25.0, 25.0), room)
                .unwrap_or_else(|| panic!("{name} is in view"));
            assert_eq!(placed.source, *want, "{name}");
        }
    }

    #[test]
    fn a_pointer_names_the_picture_point_under_it_and_a_drag_scales_to_texels() {
        let room = area(400.0, 200.0, 1.0);
        let under = point_under(point(100.0, 50.0), Permille(2000), room, (300.0, 100.0));
        // 100 px right of the middle at 2x is 50 texels.
        assert_eq!(under, point(150.0, 50.0));
        let moved = pointer_delta(Permille(2000), room, (10.0, -4.0));
        assert_eq!(moved, point(5.0, -2.0));
    }

    #[test]
    fn a_part_of_the_picture_is_the_same_part_of_a_smaller_copy_of_it() {
        let held = |w: u32, h: u32| PixelSize {
            width: PixelLen(w),
            height: PixelLen(h),
        };
        // name, part of the 400 x 200 picture, the copy's size, the same part of the copy
        type Case = (
            &'static str,
            (u32, u32, u32, u32),
            PixelSize,
            (u32, u32, u32, u32),
        );
        let cases: Vec<Case> = vec![
            (
                "a copy of the same size is the same part",
                (10, 20, 100, 50),
                SIZE,
                (10, 20, 100, 50),
            ),
            (
                "half size halves the part",
                (100, 40, 200, 100),
                held(200, 100),
                (50, 20, 100, 50),
            ),
            (
                "the whole is the whole",
                (0, 0, 400, 200),
                held(40, 20),
                (0, 0, 40, 20),
            ),
            (
                "an odd edge rounds outwards",
                (3, 3, 5, 5),
                held(200, 100),
                (1, 1, 3, 3),
            ),
            (
                "a sliver is never empty",
                (399, 199, 1, 1),
                held(4, 2),
                (3, 1, 1, 1),
            ),
        ];
        for (name, part, copy, want) in cases {
            assert_eq!(held_source(part, SIZE, copy), want, "{name}");
        }
    }

    #[test]
    fn a_pan_stops_with_the_picture_edge_flush_with_the_room() {
        // The 400 x 200 picture at 1x in a 100 x 100 room: the centre stays in 50..350 by 50..150.
        let room = area(100.0, 100.0, 1.0);
        // name, centre, the pointer's move, the centre it leaves
        type Case = (&'static str, (f64, f64), (f64, f64), (f64, f64));
        const CASES: &[Case] = &[
            (
                "a short pan is whole",
                (200.0, 100.0),
                (-30.0, 0.0),
                (230.0, 100.0),
            ),
            (
                "past the left edge ends at it",
                (60.0, 100.0),
                (500.0, 0.0),
                (50.0, 100.0),
            ),
            (
                "past the right edge ends at it",
                (340.0, 100.0),
                (-500.0, 0.0),
                (350.0, 100.0),
            ),
            (
                "past the top ends at it",
                (200.0, 60.0),
                (0.0, 500.0),
                (200.0, 50.0),
            ),
            (
                "past the bottom ends at it",
                (200.0, 140.0),
                (0.0, -500.0),
                (200.0, 150.0),
            ),
        ];
        for (name, centre, by, want) in CASES {
            let left = limited_pan(
                SIZE,
                QuarterTurn::None,
                Permille(1000),
                point(centre.0, centre.1),
                room,
                point(by.0, by.1),
            );
            let landed = point(centre.0, centre.1);
            let landed = (landed.x.0 - left.x.0, landed.y.0 - left.y.0);
            assert_eq!(
                landed,
                (point(want.0, want.1).x.0, point(want.0, want.1).y.0),
                "{name}"
            );
        }
    }

    #[test]
    fn a_picture_smaller_than_the_room_moves_only_until_its_edge_meets_the_rooms() {
        // The 400 wide picture in a 1000 wide room: its centre can lie from 400 - 500 = -100,
        // where its right edge meets the room's, to 500, where its left edge meets the room's.
        let room = area(1000.0, 100.0, 1.0);
        // name, the pointer's move, the centre it leaves
        const CASES: &[(&str, f64, f64)] = &[
            ("a short move is whole", -30.0, 230.0),
            ("towards the left edge stops there", 500.0, -100.0),
            ("towards the right edge stops there", -900.0, 500.0),
        ];
        for (name, by, want) in CASES {
            let left = limited_pan(
                SIZE,
                QuarterTurn::None,
                Permille(1000),
                point(200.0, 100.0),
                room,
                point(*by, 0.0),
            );
            let landed = point(200.0, 100.0).x.0 - left.x.0;
            assert_eq!(landed, point(*want, 0.0).x.0, "{name}");
        }
    }
    #[test]
    fn a_picture_point_is_where_the_pointer_finds_it_at_either_scale() {
        // The 400 by 200 picture in a room of 400 by 200 logical pixels. name, the room's device
        // scale, the scale it is drawn at (device pixels per texel), the room's middle as a point
        // of the picture (texels), the picture point, where that is in the room (logical pixels).
        type Case = (&'static str, f32, u32, (f64, f64), (f64, f64), (f64, f64));
        const CASES: &[Case] = &[
            (
                "scale 1, fitted: a texel is a pixel",
                1.0,
                1000,
                (200.0, 100.0),
                (100.0, 50.0),
                (100.0, 50.0),
            ),
            (
                "scale 1, its far corner",
                1.0,
                1000,
                (200.0, 100.0),
                (400.0, 200.0),
                (400.0, 200.0),
            ),
            (
                "scale 2, at actual size: a texel is half a logical pixel",
                2.0,
                1000,
                (200.0, 100.0),
                (0.0, 0.0),
                (100.0, 50.0),
            ),
            (
                "scale 2, its far corner",
                2.0,
                1000,
                (200.0, 100.0),
                (400.0, 200.0),
                (300.0, 150.0),
            ),
            (
                "scale 2, blown up four times: a texel is two logical pixels",
                2.0,
                4000,
                (100.0, 50.0),
                (110.0, 50.0),
                (220.0, 100.0),
            ),
        ];
        for (name, device, drawn, middle, texel, want) in CASES {
            let room = area(400.0, 200.0, *device);
            let at = room_point(point(middle.0, middle.1), Permille(*drawn), room, *texel);
            assert!(
                (at.0 - want.0).abs() < 0.01 && (at.1 - want.1).abs() < 0.01,
                "{name}: {at:?}"
            );
            // And the pointer there finds the same texel again.
            let back = point_under(
                point(middle.0, middle.1),
                Permille(*drawn),
                room,
                (at.0 as f32, at.1 as f32),
            );
            assert_eq!(back, point(texel.0, texel.1), "{name}: back");
        }
    }

    #[test]
    fn a_reach_of_pixels_of_the_room_is_more_texels_when_the_picture_is_shrunk() {
        // name, the scale drawn at, device scale, the reach in logical pixels, the reach in texels
        const CASES: &[(&str, u32, f32, f32, f64)] = &[
            ("actual size", 1000, 1.0, 10.0, 10.0),
            ("shrunk to half", 500, 1.0, 10.0, 20.0),
            ("blown up twice", 2000, 1.0, 10.0, 5.0),
        ];
        for (name, drawn, device, logical, texels_wanted) in CASES {
            let reach = reach_in_picture(Permille(*drawn), area(400.0, 200.0, *device), *logical);
            assert_eq!(reach, units(*texels_wanted), "{name}");
        }
        // At device scale 2 the same drawn scale covers half as many logical pixels.
        assert_eq!(
            reach_in_picture(Permille(1000), area(400.0, 200.0, 2.0), 10.0),
            units(20.0)
        );
    }

    #[test]
    fn a_rectangle_of_the_upright_part_is_found_in_the_file_unmirrored_and_offset() {
        let kept = PixelSize {
            width: PixelLen(100),
            height: PixelLen(60),
        };
        // name, reflection, the rectangle, where the part starts in the file, the file's rectangle
        type Rect = (u32, u32, u32, u32);
        type Case = (&'static str, Reflection, Rect, (u32, u32), Rect);
        const CASES: &[Case] = &[
            (
                "as it is",
                Reflection::Kept,
                (10, 20, 30, 15),
                (0, 0),
                (10, 20, 30, 15),
            ),
            (
                "mirrored, the left is the right",
                Reflection::Mirrored,
                (0, 0, 30, 15),
                (0, 0),
                (70, 0, 30, 15),
            ),
            (
                "the part starts inside the file",
                Reflection::Kept,
                (10, 20, 30, 15),
                (200, 100),
                (210, 120, 30, 15),
            ),
            (
                "mirrored and offset",
                Reflection::Mirrored,
                (10, 20, 30, 15),
                (200, 100),
                (260, 120, 30, 15),
            ),
        ];
        for (name, reflection, rect, origin, want) in CASES {
            assert_eq!(in_file(*rect, kept, *reflection, *origin), *want, "{name}");
        }
    }
}
