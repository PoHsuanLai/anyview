//! Turns and flips.

use ds_core::word::Word;

/// A clockwise rotation in whole quarter turns. Stored (it is part of an edit), so it is
/// written as its slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum QuarterTurn {
    /// Not turned.
    None,
    /// 90 degrees clockwise.
    Quarter,
    /// 180 degrees.
    Half,
    /// 270 degrees clockwise (90 counter-clockwise).
    ThreeQuarter,
}

impl QuarterTurn {
    /// The turn in degrees clockwise: 0, 90, 180 or 270.
    pub fn degrees(self) -> u16 {
        u16::from(self.quarters()) * 90
    }

    /// The turn as a count of quarter turns, 0 to 3.
    pub fn quarters(self) -> u8 {
        match self {
            QuarterTurn::None => 0,
            QuarterTurn::Quarter => 1,
            QuarterTurn::Half => 2,
            QuarterTurn::ThreeQuarter => 3,
        }
    }

    /// The turn of `quarters` quarter turns, wrapping every four.
    pub fn from_quarters(quarters: u8) -> QuarterTurn {
        match quarters % 4 {
            0 => QuarterTurn::None,
            1 => QuarterTurn::Quarter,
            2 => QuarterTurn::Half,
            _ => QuarterTurn::ThreeQuarter,
        }
    }

    /// This turn followed by `other`.
    pub fn then(self, other: QuarterTurn) -> QuarterTurn {
        QuarterTurn::from_quarters(self.quarters() + other.quarters())
    }

    /// The turn that undoes this one.
    pub fn reversed(self) -> QuarterTurn {
        QuarterTurn::from_quarters(4 - self.quarters())
    }
}

/// A line to mirror across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Axis {
    /// Left and right swap: a flip across the vertical line.
    Horizontal,
    /// Top and bottom swap: a flip across the horizontal line.
    Vertical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ds_core::testing::word_matches_serde;

    #[test]
    fn turns_add_wrap_and_reverse() {
        use QuarterTurn::{Half, None, Quarter, ThreeQuarter};
        const CASES: &[(&str, QuarterTurn, QuarterTurn, QuarterTurn, QuarterTurn)] = &[
            // name, first, second, first then second, first reversed
            ("none then quarter", None, Quarter, Quarter, None),
            ("quarter twice", Quarter, Quarter, Half, ThreeQuarter),
            (
                "three quarters then quarter wraps",
                ThreeQuarter,
                Quarter,
                None,
                Quarter,
            ),
            (
                "half then three quarters",
                Half,
                ThreeQuarter,
                Quarter,
                Half,
            ),
        ];
        for (name, a, b, sum, rev) in CASES {
            assert_eq!(a.then(*b), *sum, "{name} then");
            assert_eq!(a.reversed(), *rev, "{name} reversed");
            assert_eq!(a.then(a.reversed()), None, "{name} cancels");
        }
    }

    #[test]
    fn degrees_follow_the_quarters() {
        const CASES: &[(QuarterTurn, u16)] = &[
            (QuarterTurn::None, 0),
            (QuarterTurn::Quarter, 90),
            (QuarterTurn::Half, 180),
            (QuarterTurn::ThreeQuarter, 270),
        ];
        for (turn, degrees) in CASES {
            assert_eq!(turn.degrees(), *degrees, "{turn:?}");
        }
        assert_eq!(QuarterTurn::from_quarters(7), QuarterTurn::ThreeQuarter);
    }

    #[test]
    fn stored_words_match_their_serde_names() {
        word_matches_serde::<QuarterTurn>();
        word_matches_serde::<Axis>();
        let json = serde_json::to_string(&QuarterTurn::ThreeQuarter).unwrap();
        assert_eq!(json, "\"three_quarter\"");
        assert_eq!(
            serde_json::from_str::<QuarterTurn>(&json).unwrap(),
            QuarterTurn::ThreeQuarter
        );
    }
}
