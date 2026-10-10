//! The sheets of editing a picture: Adjust Size, and the question asked when a picture with
//! changes that are not saved is about to be left.

use crate::navigate::NavigateIn;
use anyview_core::{FilePath, PixelLen, PixelSize};

/// The most a side of a resized picture may be, so that a mistyped number cannot ask for a
/// picture no memory holds.
pub const MAX_RESIZED_SIDE: u32 = 30_000;

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
    /// The sheet for a picture that is `from` now: its own size, in pixels, keeping its
    /// proportions.
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
    pub fn size(self) -> PixelSize {
        PixelSize {
            width: PixelLen(self.width),
            height: PixelLen(self.height),
        }
    }

    /// What the numbers count.
    pub fn unit(self) -> ResizeUnit {
        self.unit
    }

    /// Whether the other side follows.
    pub fn proportion(self) -> ResizeProportion {
        self.proportion
    }

    /// The width as the field shows it: in pixels, or as a percent of the size now.
    pub fn width_shown(self) -> u32 {
        match self.unit {
            ResizeUnit::Pixels => self.width,
            ResizeUnit::Percent => {
                held(scaled(self.width, 100, u64::from(self.from.width.0.max(1))))
            }
        }
    }

    /// The height as the field shows it.
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
    pub fn changed(self, change: ResizeChange) -> ResizeDraft {
        let (from_w, from_h) = (self.from.width.0.max(1), self.from.height.0.max(1));
        match change {
            ResizeChange::Width(value) => {
                let width = self.pixels_along(value, from_w);
                let height = match self.proportion {
                    ResizeProportion::Kept => {
                        held(scaled(width, u64::from(from_h), u64::from(from_w)))
                    }
                    ResizeProportion::Free => self.height,
                };
                ResizeDraft {
                    width,
                    height,
                    ..self
                }
            }
            ResizeChange::Height(value) => {
                let height = self.pixels_along(value, from_h);
                let width = match self.proportion {
                    ResizeProportion::Kept => {
                        held(scaled(height, u64::from(from_w), u64::from(from_h)))
                    }
                    ResizeProportion::Free => self.width,
                };
                ResizeDraft {
                    width,
                    height,
                    ..self
                }
            }
            ResizeChange::Unit(unit) => ResizeDraft { unit, ..self },
            ResizeChange::ResizeProportion(proportion) => {
                let height = match proportion {
                    ResizeProportion::Kept => {
                        held(scaled(self.width, u64::from(from_h), u64::from(from_w)))
                    }
                    ResizeProportion::Free => self.height,
                };
                ResizeDraft {
                    height,
                    proportion,
                    ..self
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
    ResizeProportion(ResizeProportion),
}

/// Where the person is going when a picture with changes that are not saved is in the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureDeparture {
    /// Closing the window.
    Close,
    /// Opening this file.
    Open(FilePath),
    /// Opening these files, as the file chooser answered.
    Chosen(Vec<FilePath>),
    /// Opening these files, as dropped on the window.
    Dropped(Vec<FilePath>),
    /// Walking to another file of the list.
    Walk(NavigateIn),
}

/// Which sheet of editing a picture is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureSheet {
    /// Choosing a new size.
    Resize(ResizeDraft),
    /// Asking what to do with the changes before going where the person asked.
    Unsaved(PictureDeparture),
}

/// What moves a sheet of editing a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureSheetIn {
    /// A size changed.
    Resize(ResizeChange),
    /// Don't Save: go on without the changes.
    Decline,
}

/// What a sheet of editing a picture wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureSheetOut {
    /// Scale the picture to this size.
    Resize(PixelSize),
    /// Save the changes, then go on.
    Save(PictureDeparture),
    /// Let the changes go, and go on.
    Discard(PictureDeparture),
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
                    ResizeChange::ResizeProportion(ResizeProportion::Free),
                    ResizeChange::Width(200),
                ],
                size(200, 300),
                (200, 300),
            ),
            (
                "keeping the proportions again follows the width",
                &[
                    ResizeChange::ResizeProportion(ResizeProportion::Free),
                    ResizeChange::Width(200),
                    ResizeChange::ResizeProportion(ResizeProportion::Kept),
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
                "never above the most",
                &[ResizeChange::Width(99_999)],
                size(MAX_RESIZED_SIDE, 22_500),
                (MAX_RESIZED_SIDE, 22_500),
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
            7500,
            "30000 pixels from the longer side of 400"
        );
    }
}
