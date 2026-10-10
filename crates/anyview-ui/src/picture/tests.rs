use super::crop::{CropAspect, CropBox, CropGrip, CropLean, CropShape};
use super::{PictureEditIn, PictureEditing, PictureEdits};
use anyview_core::{
    Adjust, Axis, DocPoint, DocUnit, PixelLen, PixelRect, PixelSize, QuarterTurn, Reflection,
};

const SHOWN: PixelSize = PixelSize {
    width: PixelLen(400),
    height: PixelLen(300),
};

const fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

const fn boxed(left: u32, top: u32, width: u32, height: u32) -> CropBox {
    CropBox {
        left,
        top,
        width,
        height,
    }
}

/// A point `x`, `y` whole pixels into the picture.
const fn at(x: i32, y: i32) -> DocPoint {
    DocPoint {
        x: DocUnit(x * DocUnit::PER_PIXEL),
        y: DocUnit(y * DocUnit::PER_PIXEL),
    }
}

/// A move of `x`, `y` whole pixels, in 64ths.
const fn by(x: i64, y: i64) -> (i64, i64) {
    (x * 64, y * 64)
}

const REACH: DocUnit = DocUnit(8 * DocUnit::PER_PIXEL);

#[test]
fn a_press_takes_hold_of_the_grip_it_is_nearest() {
    // The rectangle runs from (100, 50) to (300, 150).
    let rectangle = boxed(100, 50, 200, 100);
    // name, where the press is, the grip
    const CASES: &[(&str, (i32, i32), Option<CropGrip>)] = &[
        ("top left corner", (100, 50), Some(CropGrip::NorthWest)),
        ("top right corner", (300, 50), Some(CropGrip::NorthEast)),
        ("bottom right corner", (300, 150), Some(CropGrip::SouthEast)),
        ("bottom left corner", (100, 150), Some(CropGrip::SouthWest)),
        ("top edge", (200, 50), Some(CropGrip::North)),
        ("right edge", (300, 100), Some(CropGrip::East)),
        ("bottom edge", (200, 150), Some(CropGrip::South)),
        ("left edge", (100, 100), Some(CropGrip::West)),
        ("inside", (200, 100), Some(CropGrip::Body)),
        (
            "just outside an edge still takes it",
            (96, 100),
            Some(CropGrip::West),
        ),
        (
            "a few pixels in from an edge takes it",
            (200, 145),
            Some(CropGrip::South),
        ),
        (
            "well in from an edge is the inside",
            (200, 130),
            Some(CropGrip::Body),
        ),
        ("outside past the reach takes nothing", (90, 100), None),
        ("far outside takes nothing", (10, 10), None),
    ];
    for (name, (x, y), want) in CASES {
        assert_eq!(rectangle.grip_at(at(*x, *y), REACH), *want, "{name}");
    }
}

#[test]
fn on_a_rectangle_too_small_for_its_reach_the_nearer_edge_wins() {
    let small = boxed(100, 100, 10, 10);
    assert_eq!(
        small.grip_at(at(102, 102), REACH),
        Some(CropGrip::NorthWest)
    );
    assert_eq!(
        small.grip_at(at(108, 108), REACH),
        Some(CropGrip::SouthEast)
    );
}

#[test]
fn dragging_a_grip_moves_its_sides_and_stays_inside_the_picture() {
    let start = boxed(100, 50, 200, 100);
    // name, grip, the pointer's move in whole pixels, the rectangle after
    const CASES: &[(&str, CropGrip, (i64, i64), CropBox)] = &[
        (
            "the inside moves it",
            CropGrip::Body,
            (30, 20),
            boxed(130, 70, 200, 100),
        ),
        (
            "the inside stops at the right and bottom",
            CropGrip::Body,
            (500, 500),
            boxed(200, 200, 200, 100),
        ),
        (
            "the inside stops at the left and top",
            CropGrip::Body,
            (-500, -500),
            boxed(0, 0, 200, 100),
        ),
        (
            "the right edge widens it",
            CropGrip::East,
            (50, 0),
            boxed(100, 50, 250, 100),
        ),
        (
            "the right edge stops at the picture",
            CropGrip::East,
            (500, 0),
            boxed(100, 50, 300, 100),
        ),
        (
            "the right edge stops short of the left",
            CropGrip::East,
            (-500, 0),
            boxed(100, 50, 16, 100),
        ),
        (
            "the left edge widens it leftwards",
            CropGrip::West,
            (-30, 0),
            boxed(70, 50, 230, 100),
        ),
        (
            "the left edge stops at the picture",
            CropGrip::West,
            (-500, 0),
            boxed(0, 50, 300, 100),
        ),
        (
            "the top edge",
            CropGrip::North,
            (0, -20),
            boxed(100, 30, 200, 120),
        ),
        (
            "the bottom edge stops at the picture",
            CropGrip::South,
            (0, 500),
            boxed(100, 50, 200, 250),
        ),
        (
            "a corner moves two sides",
            CropGrip::NorthWest,
            (20, 10),
            boxed(120, 60, 180, 90),
        ),
        (
            "the far corner stops at the corner of the picture",
            CropGrip::SouthEast,
            (500, 500),
            boxed(100, 50, 300, 250),
        ),
        (
            "a drag that crosses the opposite side stops at the least size",
            CropGrip::NorthWest,
            (500, 500),
            boxed(284, 134, 16, 16),
        ),
    ];
    for (name, grip, (x, y), want) in CASES {
        let got = start.dragged(*grip, by(*x, *y), SHOWN, None);
        assert_eq!(got, *want, "{name}");
    }
}

#[test]
fn a_drag_moves_by_whole_pixels_rounded_to_the_nearest() {
    let start = boxed(100, 50, 200, 100);
    // name, move in 64ths of a pixel, left after
    const CASES: &[(&str, i64, u32)] = &[
        ("under half a pixel", 31, 100),
        ("half a pixel", 32, 101),
        ("a pixel and a bit", 70, 101),
        ("back under half", -31, 100),
        ("back half", -33, 99),
    ];
    for (name, units, left) in CASES {
        assert_eq!(
            start.dragged(CropGrip::Body, (*units, 0), SHOWN, None).left,
            *left,
            "{name}"
        );
    }
}

#[test]
fn a_held_aspect_is_kept_as_a_corner_or_an_edge_moves() {
    // name, rectangle at the press, grip, move, ratio, the rectangle after
    type Case = (
        &'static str,
        CropBox,
        CropGrip,
        (i64, i64),
        (u32, u32),
        CropBox,
    );
    const CASES: &[Case] = &[
        (
            "the far corner of the whole picture, squared, narrows to the height",
            boxed(0, 0, 400, 300),
            CropGrip::SouthEast,
            (-100, 0),
            (1, 1),
            boxed(0, 0, 300, 300),
        ),
        (
            "a corner grows from the opposite corner",
            boxed(100, 50, 200, 100),
            CropGrip::NorthWest,
            (-40, 0),
            (2, 1),
            boxed(60, 30, 240, 120),
        ),
        (
            "an edge changes the other side to match, about the middle",
            boxed(100, 50, 200, 100),
            CropGrip::East,
            (40, 0),
            (2, 1),
            boxed(100, 40, 240, 120),
        ),
        (
            "the top edge changes the width to match, about the middle",
            boxed(100, 50, 200, 100),
            CropGrip::North,
            (0, -20),
            (2, 1),
            boxed(80, 30, 240, 120),
        ),
        (
            "growing past the picture stops where the ratio can still be held",
            boxed(100, 50, 200, 100),
            CropGrip::SouthEast,
            (500, 500),
            (2, 1),
            boxed(100, 50, 300, 150),
        ),
    ];
    for (name, from, grip, (x, y), ratio, want) in CASES {
        let got = from.dragged(*grip, by(*x, *y), SHOWN, Some(*ratio));
        assert_eq!(got, *want, "{name}");
        let (rw, rh) = (u64::from(ratio.0), u64::from(ratio.1));
        let (w, h) = (u64::from(got.width), u64::from(got.height));
        assert!(
            (w * rh).abs_diff(h * rw) <= rw.max(rh),
            "{name}: {got:?} is not {rw}:{rh}"
        );
        assert!(got.right() <= 400 && got.bottom() <= 300, "{name}: inside");
    }
}

#[test]
fn an_aspect_is_the_largest_rectangle_of_that_shape_inside_the_one_now() {
    let whole = CropBox::whole(SHOWN);
    // name, aspect, rectangle after
    const CASES: &[(&str, CropShape, CropLean, CropBox)] = &[
        (
            "square",
            CropShape::Square,
            CropLean::Wide,
            boxed(50, 0, 300, 300),
        ),
        (
            "four to three is the shape of the picture",
            CropShape::FourThree,
            CropLean::Wide,
            boxed(0, 0, 400, 300),
        ),
        (
            "sixteen to nine",
            CropShape::SixteenNine,
            CropLean::Wide,
            boxed(0, 37, 400, 225),
        ),
        (
            "three to four",
            CropShape::FourThree,
            CropLean::Tall,
            boxed(87, 0, 225, 300),
        ),
        (
            "nine to sixteen",
            CropShape::SixteenNine,
            CropLean::Tall,
            boxed(115, 0, 169, 300),
        ),
    ];
    for (name, shape, lean, want) in CASES {
        let ratio = CropAspect {
            shape: *shape,
            lean: *lean,
        }
        .ratio(SHOWN)
        .unwrap();
        assert_eq!(whole.shaped(ratio), *want, "{name}");
    }
}

#[test]
fn the_aspects_name_their_proportions() {
    // name, aspect, ratio for a picture of 400 by 300
    type Case = (&'static str, CropShape, CropLean, Option<(u32, u32)>);
    const CASES: &[Case] = &[
        ("free", CropShape::Free, CropLean::Wide, None),
        ("free, standing", CropShape::Free, CropLean::Tall, None),
        (
            "the original",
            CropShape::Original,
            CropLean::Wide,
            Some((400, 300)),
        ),
        ("square", CropShape::Square, CropLean::Tall, Some((1, 1))),
        (
            "four to three",
            CropShape::FourThree,
            CropLean::Wide,
            Some((4, 3)),
        ),
        (
            "three to four",
            CropShape::FourThree,
            CropLean::Tall,
            Some((3, 4)),
        ),
        (
            "sixteen to nine",
            CropShape::SixteenNine,
            CropLean::Wide,
            Some((16, 9)),
        ),
        (
            "nine to sixteen",
            CropShape::SixteenNine,
            CropLean::Tall,
            Some((9, 16)),
        ),
    ];
    for (name, shape, lean, want) in CASES {
        let aspect = CropAspect {
            shape: *shape,
            lean: *lean,
        };
        assert_eq!(aspect.ratio(SHOWN), *want, "{name}");
    }
}

#[test]
fn the_guides_cut_the_rectangle_in_thirds() {
    assert_eq!(boxed(100, 50, 200, 100).thirds(), ([166, 233], [83, 116]));
}

fn landed() -> PictureEdits {
    PictureEdits::default().step(PictureEditIn::Landed {
        size: Some(SHOWN),
        editing: PictureEditing::Allowed,
    })
}

fn run(inputs: &[PictureEditIn]) -> PictureEdits {
    inputs
        .iter()
        .fold(landed(), |picture, input| picture.step(*input))
}

const fn rectangle(left: u32, top: u32, width: u32, height: u32) -> PixelRect {
    PixelRect {
        left: PixelLen(left),
        top: PixelLen(top),
        size: size(width, height),
    }
}

const ONLY_TURNED: Adjust = Adjust {
    crop: None,
    reflection: Reflection::Kept,
    turn: QuarterTurn::Quarter,
    size: None,
};

#[test]
fn edits_fold_into_one_adjustment_that_can_be_taken_back_and_done_again() {
    use PictureEditIn::{
        Apply, CropOff, CropOn, Drag, Flip, Grab, Hold, Redo, Release, Resize, Turn, Undo,
    };
    // name, inputs, the adjustment after, whether there is an edit to take back, and to do again
    let cases: &[(&str, &[PictureEditIn], Adjust, bool, bool)] = &[
        ("nothing done", &[], Adjust::NONE, false, false),
        (
            "a turn",
            &[Turn(QuarterTurn::Quarter)],
            ONLY_TURNED,
            true,
            false,
        ),
        (
            "a turn taken back",
            &[Turn(QuarterTurn::Quarter), Undo],
            Adjust::NONE,
            false,
            true,
        ),
        (
            "a turn taken back and done again",
            &[Turn(QuarterTurn::Quarter), Undo, Redo],
            ONLY_TURNED,
            true,
            false,
        ),
        (
            "a new edit after one taken back forgets it",
            &[Turn(QuarterTurn::Quarter), Undo, Flip(Axis::Horizontal)],
            Adjust {
                reflection: Reflection::Mirrored,
                ..Adjust::NONE
            },
            true,
            false,
        ),
        (
            "taking back with nothing done does nothing",
            &[Undo, Redo],
            Adjust::NONE,
            false,
            false,
        ),
        (
            "a turn and its opposite are two edits but no change",
            &[Turn(QuarterTurn::Quarter), Turn(QuarterTurn::ThreeQuarter)],
            Adjust::NONE,
            true,
            false,
        ),
        (
            "a resize to the size it has is no resize",
            &[Resize(SHOWN)],
            Adjust::NONE,
            false,
            false,
        ),
        (
            "a resize",
            &[Resize(size(200, 150))],
            Adjust {
                size: Some(size(200, 150)),
                ..Adjust::NONE
            },
            true,
            false,
        ),
        (
            "a resize turns with the picture",
            &[Resize(size(200, 150)), Turn(QuarterTurn::Quarter)],
            Adjust {
                size: Some(size(150, 200)),
                ..ONLY_TURNED
            },
            true,
            false,
        ),
        (
            "the right edge dragged in and applied is a cut",
            &[
                CropOn,
                Grab {
                    at: at(400, 150),
                    reach: REACH,
                },
                Drag(at(300, 150)),
                Release,
                Apply,
            ],
            Adjust {
                crop: Some(rectangle(0, 0, 300, 300)),
                ..Adjust::NONE
            },
            true,
            false,
        ),
        (
            "a cut that is the whole picture cuts nothing",
            &[CropOn, Apply],
            Adjust::NONE,
            false,
            false,
        ),
        (
            "putting the tool away drops the rectangle",
            &[
                CropOn,
                Grab {
                    at: at(400, 150),
                    reach: REACH,
                },
                Drag(at(300, 150)),
                CropOff,
            ],
            Adjust::NONE,
            false,
            false,
        ),
        (
            "a cut is taken back",
            &[
                CropOn,
                Grab {
                    at: at(400, 150),
                    reach: REACH,
                },
                Drag(at(300, 150)),
                Release,
                Apply,
                Undo,
            ],
            Adjust::NONE,
            false,
            true,
        ),
        (
            "a cut after a turn is a cut of the file's own pixels",
            &[
                Turn(QuarterTurn::Quarter),
                CropOn,
                Grab {
                    at: at(150, 400),
                    reach: REACH,
                },
                Drag(at(150, 300)),
                Release,
                Apply,
            ],
            // Shown 300 wide and 400 high; the bottom edge pulled up 100 leaves the top 300 rows
            // of it. A quarter turn clockwise puts the file's left side at the top, so the file
            // keeps its columns 0 to 300 (and all 300 of its rows).
            Adjust {
                crop: Some(rectangle(0, 0, 300, 300)),
                ..ONLY_TURNED
            },
            true,
            false,
        ),
        (
            "a held aspect shapes the rectangle that is cut",
            &[
                CropOn,
                Hold(CropAspect {
                    shape: CropShape::Square,
                    lean: CropLean::Wide,
                }),
                Apply,
            ],
            Adjust {
                crop: Some(rectangle(50, 0, 300, 300)),
                ..Adjust::NONE
            },
            true,
            false,
        ),
    ];
    for (name, inputs, want, undo, redo) in cases {
        let picture = run(inputs);
        assert_eq!(picture.adjust(), *want, "{name}");
        assert_eq!(picture.can_undo(), *undo, "{name}: undo");
        assert_eq!(picture.can_redo(), *redo, "{name}: redo");
        assert_eq!(picture.is_edited(), !want.is_none(), "{name}: edited");
    }
}

#[test]
fn a_picture_that_cannot_be_saved_takes_no_edits_and_a_new_one_starts_afresh() {
    let locked = PictureEdits::default().step(PictureEditIn::Landed {
        size: Some(SHOWN),
        editing: PictureEditing::Locked,
    });
    let after = locked
        .clone()
        .step(PictureEditIn::Turn(QuarterTurn::Quarter));
    assert_eq!(after, locked, "locked, nothing changes");
    assert!(!after.can_edit());

    let edited = run(&[PictureEditIn::Turn(QuarterTurn::Quarter)]);
    assert!(edited.is_edited());
    let landed = edited.step(PictureEditIn::Landed {
        size: Some(size(10, 10)),
        editing: PictureEditing::Allowed,
    });
    assert!(
        !landed.is_edited() && !landed.can_undo(),
        "a new picture forgets"
    );
    assert_eq!(landed.base(), Some(size(10, 10)));
}

#[test]
fn the_rectangle_restarts_over_the_picture_as_it_is_turned() {
    let turned = run(&[
        PictureEditIn::CropOn,
        PictureEditIn::Turn(QuarterTurn::Quarter),
    ]);
    assert!(turned.is_cropping());
    assert_eq!(turned.draft(), Some(CropBox::whole(size(300, 400))));
    assert_eq!(turned.shown_size(), Some(size(300, 400)));
    let held = run(&[
        PictureEditIn::CropOn,
        PictureEditIn::Hold(CropAspect {
            shape: CropShape::Square,
            lean: CropLean::Wide,
        }),
        PictureEditIn::Turn(QuarterTurn::Quarter),
    ]);
    assert_eq!(held.draft(), Some(boxed(0, 50, 300, 300)), "still square");
}

#[test]
fn only_a_press_on_the_rectangle_starts_a_drag_and_a_release_ends_it() {
    let picture = run(&[
        PictureEditIn::CropOn,
        PictureEditIn::Grab {
            at: at(400, 150),
            reach: REACH,
        },
    ]);
    assert!(picture.is_dragging());
    assert!(!picture.step(PictureEditIn::Release).is_dragging());
    let outside = run(&[
        PictureEditIn::CropOn,
        PictureEditIn::Grab {
            at: at(100, 100),
            reach: DocUnit(0),
        },
    ]);
    assert!(
        outside.is_dragging(),
        "the inside of the whole picture is the rectangle's body"
    );
    let shrunk = run(&[
        PictureEditIn::CropOn,
        PictureEditIn::Hold(CropAspect {
            shape: CropShape::Square,
            lean: CropLean::Wide,
        }),
        PictureEditIn::Grab {
            at: at(5, 5),
            reach: REACH,
        },
    ]);
    assert!(
        !shrunk.is_dragging(),
        "a press outside the rectangle takes nothing"
    );
}
