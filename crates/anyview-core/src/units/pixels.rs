//! Sizes in device pixels.

/// A length in pixels.
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
pub struct PixelLen(pub u32);

/// A pixel count: the area of an image, or a budget for how many pixels may be decoded.
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
pub struct PixelArea(pub u64);

/// A width and a height in pixels.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct PixelSize {
    /// Columns of pixels.
    pub width: PixelLen,
    /// Rows of pixels.
    pub height: PixelLen,
}

impl PixelSize {
    /// The pixels the size covers.
    pub fn area(self) -> PixelArea {
        PixelArea(u64::from(self.width.0) * u64::from(self.height.0)) // two u32s cannot overflow a u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_is_width_times_height() {
        const CASES: &[(&str, u32, u32, u64)] = &[
            ("empty", 0, 10, 0),
            ("hd", 1920, 1080, 2_073_600),
            ("beyond u32", u32::MAX, u32::MAX, 18_446_744_065_119_617_025),
        ];
        for (name, w, h, area) in CASES {
            let size = PixelSize {
                width: PixelLen(*w),
                height: PixelLen(*h),
            };
            assert_eq!(size.area(), PixelArea(*area), "{name}");
        }
    }

    #[test]
    fn a_size_round_trips() {
        let size = PixelSize {
            width: PixelLen(640),
            height: PixelLen(480),
        };
        let json = serde_json::to_string(&size).unwrap();
        assert_eq!(json, r#"{"width":640,"height":480}"#);
        assert_eq!(serde_json::from_str::<PixelSize>(&json).unwrap(), size);
    }
}
