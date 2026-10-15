//! Tests of where a picture lands in the room.

use super::place::{held_source, in_file, place};
use super::pointer::{limited_pan, point_under, pointer_delta, reach_in_picture, room_point};
use super::scale::{fit, units};
use crate::Area;
use anyview_core::{DocPoint, Permille, PixelSize, QuarterTurn, Reflection};

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
