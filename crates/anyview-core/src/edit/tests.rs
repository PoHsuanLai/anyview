use super::*;
use crate::units::MediaTime;
use crate::units::{PixelLen, PixelSize};
use ds_core::testing::word_matches_serde;

fn range(first: u32, last: u32) -> PageRange {
    PageRange::new(PageIndex(first), PageIndex(last)).unwrap()
}

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

fn rect(left: u32, top: u32, width: u32, height: u32) -> PixelRect {
    PixelRect {
        left: PixelLen(left),
        top: PixelLen(top),
        size: size(width, height),
    }
}

#[test]
fn each_edit_reports_its_kind() {
    let cases = [
        (Edit::Rotate(QuarterTurn::Quarter), EditKind::Rotate),
        (Edit::Flip(Axis::Vertical), EditKind::Flip),
        (Edit::DeletePages(range(1, 2)), EditKind::DeletePages),
        (
            Edit::MovePage {
                from: PageIndex(0),
                to: PageIndex(3),
            },
            EditKind::MovePage,
        ),
        (Edit::Adjust(Adjust::NONE), EditKind::Adjust),
        (
            Edit::Trim(TimeRange::new(MediaTime(1), Some(MediaTime(2))).unwrap()),
            EditKind::Trim,
        ),
        (Edit::Markup, EditKind::Markup),
    ];
    for (edit, kind) in cases {
        assert_eq!(edit.kind(), kind, "{edit:?}");
    }
}

#[test]
fn edits_round_trip_adjacently_tagged() {
    let cases = [
        (
            Edit::Rotate(QuarterTurn::Half),
            r#"{"kind":"rotate","v":"half"}"#,
        ),
        (
            Edit::Flip(Axis::Horizontal),
            r#"{"kind":"flip","v":"horizontal"}"#,
        ),
        (
            Edit::DeletePages(range(2, 4)),
            r#"{"kind":"delete_pages","v":{"first":2,"last":4}}"#,
        ),
        (
            Edit::MovePage {
                from: PageIndex(1),
                to: PageIndex(5),
            },
            r#"{"kind":"move_page","v":{"from":1,"to":5}}"#,
        ),
        (Edit::Markup, r#"{"kind":"markup"}"#),
        (
            Edit::Adjust(Adjust::NONE),
            r#"{"kind":"adjust","v":{"crop":null,"reflection":"kept","turn":"none","size":null}}"#,
        ),
        (
            Edit::Adjust(Adjust {
                crop: Some(rect(1, 2, 30, 40)),
                reflection: Reflection::Mirrored,
                turn: QuarterTurn::Quarter,
                size: Some(size(15, 20)),
            }),
            r#"{"kind":"adjust","v":{"crop":{"left":1,"top":2,"size":{"width":30,"height":40}},"reflection":"mirrored","turn":"quarter","size":{"width":15,"height":20}}}"#,
        ),
        (
            Edit::Trim(TimeRange::new(MediaTime(1_500_000), Some(MediaTime(2_500_000))).unwrap()),
            r#"{"kind":"trim","v":{"start":1500000,"end":2500000}}"#,
        ),
    ];
    for (edit, json) in cases {
        assert_eq!(serde_json::to_string(&edit).unwrap(), json, "{edit:?}");
        assert_eq!(
            serde_json::from_str::<Edit>(json).unwrap(),
            edit,
            "{edit:?}"
        );
    }
}

#[test]
fn edit_kinds_are_stored_as_their_slugs() {
    word_matches_serde::<EditKind>();
    word_matches_serde::<Reflection>();
}

#[test]
fn turns_and_flips_fold_into_one_adjustment() {
    use QuarterTurn::{Half, Quarter, ThreeQuarter};
    #[derive(Clone, Copy)]
    enum Step {
        Turn(QuarterTurn),
        Flip(Axis),
    }
    use Step::{Flip, Turn};
    // name, steps, the mirror and turn they make
    const CASES: &[(&str, &[Step], Reflection, QuarterTurn)] = &[
        ("a quarter", &[Turn(Quarter)], Reflection::Kept, Quarter),
        (
            "a quarter back",
            &[Turn(Quarter), Turn(ThreeQuarter)],
            Reflection::Kept,
            QuarterTurn::None,
        ),
        (
            "a flip across the vertical line",
            &[Flip(Axis::Horizontal)],
            Reflection::Mirrored,
            QuarterTurn::None,
        ),
        (
            "that flip twice",
            &[Flip(Axis::Horizontal), Flip(Axis::Horizontal)],
            Reflection::Kept,
            QuarterTurn::None,
        ),
        (
            "a flip across the horizontal line is a mirror and a half turn",
            &[Flip(Axis::Vertical)],
            Reflection::Mirrored,
            Half,
        ),
        (
            "a quarter, then a flip across the vertical line",
            &[Turn(Quarter), Flip(Axis::Horizontal)],
            Reflection::Mirrored,
            ThreeQuarter,
        ),
        (
            "a flip, then a quarter keeps the mirror",
            &[Flip(Axis::Horizontal), Turn(Quarter)],
            Reflection::Mirrored,
            Quarter,
        ),
    ];
    for (name, steps, reflection, turn) in CASES {
        let made = steps.iter().fold(Adjust::NONE, |a, step| match step {
            Turn(by) => a.turned(*by),
            Flip(axis) => a.flipped(*axis),
        });
        assert_eq!((made.reflection, made.turn), (*reflection, *turn), "{name}");
    }
}

#[test]
fn a_chosen_size_turns_with_the_picture_and_a_flip_leaves_it() {
    let sized = Adjust {
        size: Some(size(30, 20)),
        ..Adjust::NONE
    };
    assert_eq!(sized.turned(QuarterTurn::Quarter).size, Some(size(20, 30)));
    assert_eq!(sized.turned(QuarterTurn::Half).size, Some(size(30, 20)));
    assert_eq!(sized.flipped(Axis::Vertical).size, Some(size(30, 20)));
}

#[test]
fn sizes_follow_the_crop_the_turn_and_the_resize() {
    let base = size(100, 60);
    // name, adjustment, size before a resize, size of the result
    let cases = [
        ("untouched", Adjust::NONE, size(100, 60), size(100, 60)),
        (
            "turned a quarter",
            Adjust::NONE.turned(QuarterTurn::Quarter),
            size(60, 100),
            size(60, 100),
        ),
        (
            "cut",
            Adjust {
                crop: Some(rect(10, 10, 40, 20)),
                ..Adjust::NONE
            },
            size(40, 20),
            size(40, 20),
        ),
        (
            "cut, turned and resized",
            Adjust {
                crop: Some(rect(10, 10, 40, 20)),
                turn: QuarterTurn::ThreeQuarter,
                size: Some(size(10, 20)),
                ..Adjust::NONE
            },
            size(20, 40),
            size(10, 20),
        ),
    ];
    for (name, adjust, turned, result) in cases {
        assert_eq!(adjust.turned_size(base), turned, "{name}: turned");
        assert_eq!(adjust.result_size(base), result, "{name}: result");
    }
}

#[test]
fn a_rectangle_of_what_is_shown_is_cut_out_of_the_whole_picture() {
    // A picture 100 wide and 60 high. name, what was done first, the rectangle cut out of
    // what is shown, the crop of the whole picture it makes.
    let base = size(100, 60);
    let cases = [
        (
            "untouched",
            Adjust::NONE,
            rect(10, 20, 30, 15),
            rect(10, 20, 30, 15),
        ),
        (
            "mirrored: the left of the shown is the right of the file",
            Adjust::NONE.flipped(Axis::Horizontal),
            rect(0, 0, 30, 15),
            rect(70, 0, 30, 15),
        ),
        (
            "turned a quarter: the top of the shown is the left of the file",
            Adjust::NONE.turned(QuarterTurn::Quarter),
            rect(0, 0, 20, 30),
            rect(0, 40, 30, 20),
        ),
        (
            "turned three quarters: the top of the shown is the right of the file",
            Adjust::NONE.turned(QuarterTurn::ThreeQuarter),
            rect(0, 0, 20, 30),
            rect(70, 0, 30, 20),
        ),
        (
            "turned half: the shown's corner is the opposite one",
            Adjust::NONE.turned(QuarterTurn::Half),
            rect(0, 0, 30, 15),
            rect(70, 45, 30, 15),
        ),
        (
            "cut before: the new rectangle is inside the old",
            Adjust {
                crop: Some(rect(20, 10, 50, 40)),
                ..Adjust::NONE
            },
            rect(5, 5, 10, 10),
            rect(25, 15, 10, 10),
        ),
    ];
    for (name, before, shown, want) in cases {
        let made = before.cropped(base, shown);
        assert_eq!(made.crop, Some(want), "{name}");
        assert_eq!(made.size, None, "{name}: a resize before is let go");
    }
}

#[test]
fn a_rectangle_past_the_edge_is_held_inside_the_picture() {
    let made = Adjust::NONE.cropped(size(100, 60), rect(90, 50, 30, 30));
    assert_eq!(made.crop, Some(rect(90, 50, 10, 10)));
}
