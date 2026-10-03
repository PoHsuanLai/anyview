//! Places inside a piece of content: a point in its own space and a line of text.

/// A length in the content's own space, in 1/64 of one content pixel, so panning is exact at any
/// zoom and the value stays an integer.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct DocUnit(pub i32);

impl DocUnit {
    /// Units in one content pixel.
    pub const PER_PIXEL: i32 = 64;

    /// `pixels` whole content pixels, saturating at the largest representable length.
    pub fn from_pixels(pixels: i32) -> DocUnit {
        DocUnit(pixels.saturating_mul(Self::PER_PIXEL))
    }

    /// The length in whole content pixels, rounded toward negative infinity.
    pub fn whole_pixels(self) -> i32 {
        self.0.div_euclid(Self::PER_PIXEL)
    }
}

/// A point in the content's own space: what an image or a page keeps as its view centre.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct DocPoint {
    /// Distance from the content's left edge.
    pub x: DocUnit,
    /// Distance from the content's top edge.
    pub y: DocUnit,
}

/// A line of a text file, counting from zero.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct LineIndex(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_convert_to_units_and_back() {
        const CASES: &[(&str, i32, i32, i32)] = &[
            // name, pixels, units, whole pixels back
            ("zero", 0, 0, 0),
            ("positive", 3, 192, 3),
            ("negative", -2, -128, -2),
        ];
        for (name, pixels, units, back) in CASES {
            assert_eq!(DocUnit::from_pixels(*pixels), DocUnit(*units), "{name}");
            assert_eq!(DocUnit(*units).whole_pixels(), *back, "{name}");
        }
    }

    #[test]
    fn whole_pixels_round_down_and_pixels_saturate() {
        assert_eq!(DocUnit(63).whole_pixels(), 0);
        assert_eq!(DocUnit(-1).whole_pixels(), -1);
        assert_eq!(DocUnit::from_pixels(i32::MAX), DocUnit(i32::MAX));
    }

    #[test]
    fn a_point_and_a_line_round_trip() {
        let point = DocPoint {
            x: DocUnit(-5),
            y: DocUnit(640),
        };
        let json = serde_json::to_string(&point).unwrap();
        assert_eq!(json, r#"{"x":-5,"y":640}"#);
        assert_eq!(serde_json::from_str::<DocPoint>(&json).unwrap(), point);
        assert_eq!(
            serde_json::from_str::<LineIndex>("12").unwrap(),
            LineIndex(12)
        );
    }
}
