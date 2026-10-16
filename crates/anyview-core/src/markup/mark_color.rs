//! The palette a mark is coloured from.

use ds_core::word::Word;

/// The colours a mark can have: the palette Preview offers, yellow first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum MarkColor {
    /// The highlighter's own colour.
    #[default]
    Yellow,
    /// Orange.
    Orange,
    /// Red.
    Red,
    /// Pink.
    Pink,
    /// Purple.
    Purple,
    /// Blue.
    Blue,
    /// Green.
    Green,
    /// Black.
    Black,
}

impl MarkColor {
    /// The colour as red, green and blue.
    #[must_use]
    pub const fn rgb(self) -> (u8, u8, u8) {
        match self {
            MarkColor::Yellow => (255, 214, 10),
            MarkColor::Orange => (255, 159, 10),
            MarkColor::Red => (255, 69, 58),
            MarkColor::Pink => (255, 105, 180),
            MarkColor::Purple => (191, 90, 242),
            MarkColor::Blue => (10, 132, 255),
            MarkColor::Green => (48, 209, 88),
            MarkColor::Black => (0, 0, 0),
        }
    }

    /// The palette colour closest to this red, green and blue.
    #[must_use]
    pub fn nearest(red: u8, green: u8, blue: u8) -> MarkColor {
        let apart = |color: &MarkColor| {
            let (r, g, b) = color.rgb();
            let square = |a: u8, b: u8| u32::from(a.abs_diff(b)).pow(2);
            square(r, red) + square(g, green) + square(b, blue)
        };
        [
            MarkColor::Yellow,
            MarkColor::Orange,
            MarkColor::Red,
            MarkColor::Pink,
            MarkColor::Purple,
            MarkColor::Blue,
            MarkColor::Green,
            MarkColor::Black,
        ]
        .into_iter()
        .min_by_key(apart)
        .unwrap_or_default()
    }
}
