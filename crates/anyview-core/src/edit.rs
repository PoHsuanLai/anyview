//! Edits: each change the viewer can save in place, described as data.

use crate::units::{Axis, PageIndex, PageRange, PixelLen, PixelSize, QuarterTurn};
use ds_core::word::Word;

/// One change to a file. An edit is a value: the save pipeline applies it, undo stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Edit {
    /// Turn the content clockwise by this much.
    Rotate(QuarterTurn),
    /// Mirror the content across an axis.
    Flip(Axis),
    /// Remove a run of pages.
    DeletePages(PageRange),
    /// Move one page to a new place.
    MovePage {
        /// The page's place now.
        from: PageIndex,
        /// The place it moves to.
        to: PageIndex,
    },
    /// Everything done to a picture since it was opened, as one change: cut down, mirrored,
    /// turned and resized.
    Adjust(Adjust),
}

/// Whether a picture is mirrored left to right before it is turned.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize, Word,
)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Reflection {
    /// As it was.
    #[default]
    Kept,
    /// Left and right swapped.
    Mirrored,
}

/// A rectangle of a picture's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PixelRect {
    /// Columns from the left edge to the rectangle's.
    pub left: PixelLen,
    /// Rows from the top edge to the rectangle's.
    pub top: PixelLen,
    /// Its width and height.
    pub size: PixelSize,
}

/// What has been done to a picture, in the order it is applied to the pixels of the file as it
/// is decoded (already upright): cut to `crop`, mirrored, turned, then resized to `size`. Doing
/// any of it again later folds into the same value, so a series of edits is one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Adjust {
    /// The part of the picture that is kept; the whole of it when none.
    pub crop: Option<PixelRect>,
    /// Whether the kept part is mirrored before it is turned.
    pub reflection: Reflection,
    /// How far the kept part is turned clockwise.
    pub turn: QuarterTurn,
    /// The size the result is scaled to; the kept part's own when none.
    pub size: Option<PixelSize>,
}

impl Adjust {
    /// Nothing done.
    pub const NONE: Adjust = Adjust {
        crop: None,
        reflection: Reflection::Kept,
        turn: QuarterTurn::None,
        size: None,
    };

    /// Whether nothing has been done.
    pub fn is_none(&self) -> bool {
        *self == Adjust::NONE
    }

    /// This, and then the picture as shown is turned clockwise by `by`. A size chosen with
    /// Adjust Size turns with the picture.
    pub fn turned(self, by: QuarterTurn) -> Adjust {
        let swaps = matches!(by, QuarterTurn::Quarter | QuarterTurn::ThreeQuarter);
        Adjust {
            turn: self.turn.then(by),
            size: self.size.map(|size| {
                if swaps {
                    PixelSize {
                        width: size.height,
                        height: size.width,
                    }
                } else {
                    size
                }
            }),
            ..self
        }
    }

    /// This, and then the picture as shown is mirrored across `axis`. A mirror then a turn,
    /// mirrored again, is the opposite mirror and the reverse turn; a flip across the horizontal
    /// line is that and a half turn more.
    pub fn flipped(self, axis: Axis) -> Adjust {
        let reflection = match self.reflection {
            Reflection::Kept => Reflection::Mirrored,
            Reflection::Mirrored => Reflection::Kept,
        };
        let across_vertical = Adjust {
            reflection,
            turn: self.turn.reversed(),
            ..self
        };
        match axis {
            Axis::Horizontal => across_vertical,
            Axis::Vertical => Adjust {
                turn: across_vertical.turn.then(QuarterTurn::Half),
                ..across_vertical
            },
        }
    }

    /// The part of a picture of size `base` that is kept.
    pub fn kept(&self, base: PixelSize) -> PixelRect {
        self.crop.unwrap_or(PixelRect {
            left: PixelLen(0),
            top: PixelLen(0),
            size: base,
        })
    }

    /// The size of the kept part once it is turned, before any resize: what the person sees
    /// while they choose a new crop.
    pub fn turned_size(&self, base: PixelSize) -> PixelSize {
        let kept = self.kept(base).size;
        match self.turn {
            QuarterTurn::None | QuarterTurn::Half => kept,
            QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => PixelSize {
                width: kept.height,
                height: kept.width,
            },
        }
    }

    /// The size of the result.
    pub fn result_size(&self, base: PixelSize) -> PixelSize {
        self.size.unwrap_or_else(|| self.turned_size(base))
    }

    /// This with `shown`, a rectangle of the picture as it is shown now (kept, mirrored and
    /// turned, before any resize), cut out of it. The new crop is a rectangle of the whole
    /// picture, so a mirror or a turn made before or after does not move it. The rectangle is
    /// held inside the picture, and a resize chosen earlier is let go: it was a size for the
    /// picture before it was cut.
    pub fn cropped(self, base: PixelSize, shown: PixelRect) -> Adjust {
        let kept = self.kept(base);
        let (width, height) = (kept.size.width.0, kept.size.height.0);
        let (x, y) = (shown.left.0, shown.top.0);
        let (w, h) = (shown.size.width.0, shown.size.height.0);
        // Undo the turn: the rectangle of the mirrored part it was cut from.
        let (mx, my, mw, mh) = match self.turn {
            QuarterTurn::None => (x, y, w, h),
            QuarterTurn::Quarter => (y, height.saturating_sub(x + w), h, w),
            QuarterTurn::Half => (
                width.saturating_sub(x + w),
                height.saturating_sub(y + h),
                w,
                h,
            ),
            QuarterTurn::ThreeQuarter => (width.saturating_sub(y + h), x, h, w),
        };
        // Undo the mirror.
        let mx = match self.reflection {
            Reflection::Kept => mx,
            Reflection::Mirrored => width.saturating_sub(mx + mw),
        };
        let mw = mw.min(width.saturating_sub(mx));
        let mh = mh.min(height.saturating_sub(my));
        Adjust {
            crop: Some(PixelRect {
                left: PixelLen(kept.left.0 + mx),
                top: PixelLen(kept.top.0 + my),
                size: PixelSize {
                    width: PixelLen(mw),
                    height: PixelLen(mh),
                },
            }),
            size: None,
            ..self
        }
    }
}

impl Default for Adjust {
    fn default() -> Self {
        Adjust::NONE
    }
}

/// The sorts of edit, without their options: what a kind of file offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum EditKind {
    /// [`Edit::Rotate`].
    Rotate,
    /// [`Edit::Flip`].
    Flip,
    /// [`Edit::DeletePages`].
    DeletePages,
    /// [`Edit::MovePage`].
    MovePage,
    /// [`Edit::Adjust`].
    Adjust,
}

impl Edit {
    /// Which sort of edit this is.
    pub fn kind(&self) -> EditKind {
        match self {
            Edit::Rotate(_) => EditKind::Rotate,
            Edit::Flip(_) => EditKind::Flip,
            Edit::DeletePages(_) => EditKind::DeletePages,
            Edit::MovePage { .. } => EditKind::MovePage,
            Edit::Adjust(_) => EditKind::Adjust,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
