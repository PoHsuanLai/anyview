//! What has been done to a picture: the crop, the mirror, the turn and the size.

use crate::units::{Axis, PixelLen, PixelSize, QuarterTurn};
use ds_core::word::Word;

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
