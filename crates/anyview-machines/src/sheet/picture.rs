//! The sheets of editing a picture: Adjust Size, and the question asked when a picture with
//! changes that are not saved is about to be left.

use super::option::MAX_LONG_EDGE;
use anyview_core::{PixelLen, PixelSize};

/// The most a side of a resized picture may be: the longest side an export may ask for, so that
/// the two agree.
pub const MAX_RESIZED_SIDE: u32 = MAX_LONG_EDGE.0;

/// The most pixels a resized picture may hold (400 MB as 8-bit RGBA), so that a mistyped number
/// cannot ask for a picture no memory holds. A side cap alone would allow a square of
/// 16 384 pixels, a gigabyte.
pub const MAX_RESIZED_AREA: u64 = 100_000_000;

/// What the numbers of Adjust Size count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ResizeUnit {
    /// Pixels.
    #[default]
    Pixels,
    /// A share of the size the picture has now.
    Percent,
}

impl ResizeUnit {
    /// What the control calls it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ResizeUnit::Pixels => "Pixels",
            ResizeUnit::Percent => "Percent",
        }
    }
}

/// Whether the other side follows when one is typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ResizeProportion {
    /// The picture keeps its proportions.
    #[default]
    Kept,
    /// Each side is its own.
    Free,
}

/// The sizes being chosen in Adjust Size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResizeDraft {
    from: PixelSize,
    width: u32,
    height: u32,
    unit: ResizeUnit,
    proportion: ResizeProportion,
}

/// `value` scaled by `numerator` over `denominator`, to the nearest whole number.
fn scaled(value: u32, numerator: u64, denominator: u64) -> u64 {
    (u64::from(value) * numerator + denominator / 2) / denominator.max(1)
}

fn held(value: u64) -> u32 {
    u32::try_from(value.clamp(1, u64::from(MAX_RESIZED_SIDE))).unwrap_or(MAX_RESIZED_SIDE)
}

impl ResizeDraft {
    /// The two sides when one, `typed`, was set and the other was `other`: the draft's sides
    /// brought inside `MAX_RESIZED_AREA`. `from` is the original `(typed, other)` sides. Kept
    /// proportions shrink the typed side, so that the other keeps following it; free sides shrink
    /// the other one.
    fn within_area(self, typed: u32, other: u32, from: (u32, u32)) -> (u32, u32) {
        let (typed, other) = match self.proportion {
            ResizeProportion::Kept => {
                let room = MAX_RESIZED_AREA * u64::from(from.0) / u64::from(from.1);
                let typed = typed.min(u32::try_from(room.isqrt()).unwrap_or(MAX_RESIZED_SIDE));
                let typed = typed.max(1);
                (
                    typed,
                    held(scaled(typed, u64::from(from.1), u64::from(from.0))),
                )
            }
            ResizeProportion::Free => (typed, other),
        };
        // Rounding can leave the area a little over; the other side gives way.
        let room = MAX_RESIZED_AREA / u64::from(typed);
        (typed, held(u64::from(other).min(room)))
    }

    /// The sheet for a picture that is `from` now: its own size, in pixels, keeping its
    /// proportions.
    #[must_use]
    pub fn of(from: PixelSize) -> ResizeDraft {
        ResizeDraft {
            from,
            width: from.width.0.max(1),
            height: from.height.0.max(1),
            unit: ResizeUnit::Pixels,
            proportion: ResizeProportion::Kept,
        }
    }

    /// The size the picture would become.
    #[must_use]
    pub fn size(self) -> PixelSize {
        PixelSize {
            width: PixelLen(self.width),
            height: PixelLen(self.height),
        }
    }

    /// What the numbers count.
    #[must_use]
    pub fn unit(self) -> ResizeUnit {
        self.unit
    }

    /// Whether the other side follows.
    #[must_use]
    pub fn proportion(self) -> ResizeProportion {
        self.proportion
    }

    /// The width as the field shows it: in pixels, or as a percent of the size now.
    #[must_use]
    pub fn width_shown(self) -> u32 {
        match self.unit {
            ResizeUnit::Pixels => self.width,
            ResizeUnit::Percent => {
                held(scaled(self.width, 100, u64::from(self.from.width.0.max(1))))
            }
        }
    }

    /// The height as the field shows it.
    #[must_use]
    pub fn height_shown(self) -> u32 {
        match self.unit {
            ResizeUnit::Pixels => self.height,
            ResizeUnit::Percent => held(scaled(
                self.height,
                100,
                u64::from(self.from.height.0.max(1)),
            )),
        }
    }

    /// The most a field accepts, in the unit it counts.
    #[must_use]
    pub fn most(self) -> u32 {
        match self.unit {
            ResizeUnit::Pixels => MAX_RESIZED_SIDE,
            ResizeUnit::Percent => held(scaled(
                MAX_RESIZED_SIDE,
                100,
                u64::from(self.from.width.0.max(self.from.height.0).max(1)),
            )),
        }
    }

    /// `value`, a number in the unit now, as pixels along a side that is `side` long.
    fn pixels_along(self, value: u32, side: u32) -> u32 {
        match self.unit {
            ResizeUnit::Pixels => held(u64::from(value)),
            ResizeUnit::Percent => held(scaled(side.max(1), u64::from(value), 100)),
        }
    }

    /// The draft after `change`.
    #[must_use]
    pub fn changed(self, change: ResizeChange) -> ResizeDraft {
        let (from_w, from_h) = (self.from.width.0.max(1), self.from.height.0.max(1));
        match change {
            ResizeChange::Width(value) => {
                let typed = self.pixels_along(value, from_w);
                let (width, height) = self.within_area(typed, self.height, (from_w, from_h));
                ResizeDraft {
                    width,
                    height,
                    ..self
                }
            }
            ResizeChange::Height(value) => {
                let typed = self.pixels_along(value, from_h);
                let (height, width) = self.within_area(typed, self.width, (from_h, from_w));
                ResizeDraft {
                    width,
                    height,
                    ..self
                }
            }
            ResizeChange::Unit(unit) => ResizeDraft { unit, ..self },
            ResizeChange::Proportion(proportion) => {
                let draft = ResizeDraft { proportion, ..self };
                let (width, height) = draft.within_area(self.width, self.height, (from_w, from_h));
                ResizeDraft {
                    width,
                    height,
                    ..draft
                }
            }
        }
    }
}

/// One change to the sizes in Adjust Size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeChange {
    /// A width was typed or stepped, in the unit now.
    Width(u32),
    /// A height was typed or stepped, in the unit now.
    Height(u32),
    /// The numbers now count this.
    Unit(ResizeUnit),
    /// The sides follow each other, or do not.
    Proportion(ResizeProportion),
}

/// Which sheet of editing a picture is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureSheet {
    /// Choosing a new size.
    Resize(ResizeDraft),
}

/// What moves a sheet of editing a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureSheetIn {
    /// A size changed.
    Resize(ResizeChange),
}

/// What a sheet of editing a picture wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureSheetOut {
    /// Scale the picture to this size.
    Resize(PixelSize),
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn size(width: u32, height: u32) -> PixelSize {
        PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        }
    }

    #[test]
    fn the_sizes_follow_each_other_while_the_proportions_are_kept() {
        let from = size(400, 300);
        // name, changes, the size after, the numbers shown (width, height)
        type Case = (&'static str, &'static [ResizeChange], PixelSize, (u32, u32));
        const CASES: &[Case] = &[
            ("as it is", &[], size(400, 300), (400, 300)),
            (
                "a typed width brings its height",
                &[ResizeChange::Width(200)],
                size(200, 150),
                (200, 150),
            ),
            (
                "a typed height brings its width",
                &[ResizeChange::Height(60)],
                size(80, 60),
                (80, 60),
            ),
            (
                "rounded to the nearest pixel",
                &[ResizeChange::Width(101)],
                size(101, 76),
                (101, 76),
            ),
            (
                "free sides are their own",
                &[
                    ResizeChange::Proportion(ResizeProportion::Free),
                    ResizeChange::Width(200),
                ],
                size(200, 300),
                (200, 300),
            ),
            (
                "keeping the proportions again follows the width",
                &[
                    ResizeChange::Proportion(ResizeProportion::Free),
                    ResizeChange::Width(200),
                    ResizeChange::Proportion(ResizeProportion::Kept),
                ],
                size(200, 150),
                (200, 150),
            ),
            (
                "percent counts the size now",
                &[
                    ResizeChange::Unit(ResizeUnit::Percent),
                    ResizeChange::Width(50),
                ],
                size(200, 150),
                (50, 50),
            ),
            (
                "percent, switched back to pixels, shows the pixels",
                &[
                    ResizeChange::Unit(ResizeUnit::Percent),
                    ResizeChange::Width(25),
                    ResizeChange::Unit(ResizeUnit::Pixels),
                ],
                size(100, 75),
                (100, 75),
            ),
            (
                "never below a pixel",
                &[ResizeChange::Width(0)],
                size(1, 1),
                (1, 1),
            ),
            (
                "never above the most, kept in proportion within the area",
                &[ResizeChange::Width(99_999)],
                size(11_547, 8660),
                (11_547, 8660),
            ),
            (
                "free sides give way to each other past the area",
                &[
                    ResizeChange::Proportion(ResizeProportion::Free),
                    ResizeChange::Width(16_000),
                    ResizeChange::Height(16_000),
                ],
                size(6250, 16_000),
                (6250, 16_000),
            ),
        ];
        for (name, changes, want, shown) in CASES {
            let draft = changes.iter().fold(ResizeDraft::of(from), |draft, change| {
                draft.changed(*change)
            });
            assert_eq!(draft.size(), *want, "{name}");
            assert_eq!(
                (draft.width_shown(), draft.height_shown()),
                *shown,
                "{name}"
            );
        }
    }

    #[test]
    fn a_field_accepts_up_to_the_most_a_side_can_be() {
        let draft = ResizeDraft::of(size(400, 300));
        assert_eq!(draft.most(), MAX_RESIZED_SIDE);
        let percent = draft.changed(ResizeChange::Unit(ResizeUnit::Percent));
        assert_eq!(
            percent.most(),
            4096,
            "16384 pixels from the longer side of 400"
        );
    }
}
