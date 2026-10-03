//! Proportions: whole percent, thousandths, and the 1 to 100 quality of a lossy encode.

/// A whole-number percentage. Not clamped by the type: a volume goes to 150 and a bitrate gain
/// further, so each user (`Volume`, `Quality`) says its own range and clamps.
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
pub struct Percent(pub u16);

/// A proportion in thousandths: 1000 is the whole. Zoom uses it, so 1000 is "actual size".
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
pub struct Permille(pub u32);

impl Permille {
    /// The whole.
    pub const WHOLE: Permille = Permille(1000);
}

/// How much detail a lossy encode keeps, 1 to 100.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(from = "Percent", into = "Percent")]
pub struct Quality(Percent);

impl Quality {
    /// The lowest quality an encoder accepts.
    pub const MIN: Quality = Quality(Percent(1));
    /// The highest quality.
    pub const MAX: Quality = Quality(Percent(100));

    /// `percent` clamped into 1 to 100: zero becomes 1 and anything above becomes 100.
    pub fn clamped(percent: Percent) -> Self {
        Quality(Percent(percent.0.clamp(Self::MIN.0.0, Self::MAX.0.0)))
    }

    /// The quality as a percentage, always 1 to 100.
    pub fn percent(self) -> Percent {
        self.0
    }
}

impl From<Percent> for Quality {
    fn from(percent: Percent) -> Self {
        Quality::clamped(percent)
    }
}

impl From<Quality> for Percent {
    fn from(quality: Quality) -> Self {
        quality.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_clamps_into_one_to_one_hundred() {
        const CASES: &[(&str, u16, u16)] = &[
            ("zero rises to one", 0, 1),
            ("lowest", 1, 1),
            ("middle", 80, 80),
            ("highest", 100, 100),
            ("above falls to one hundred", 101, 100),
            ("far above", u16::MAX, 100),
        ];
        for (name, given, want) in CASES {
            assert_eq!(
                Quality::clamped(Percent(*given)).percent(),
                Percent(*want),
                "{name}"
            );
        }
    }

    #[test]
    fn quality_loads_clamped_and_round_trips() {
        let q = Quality::clamped(Percent(85));
        let json = serde_json::to_string(&q).unwrap();
        assert_eq!(json, "85");
        assert_eq!(serde_json::from_str::<Quality>(&json).unwrap(), q);
        assert_eq!(serde_json::from_str::<Quality>("0").unwrap(), Quality::MIN);
        assert_eq!(
            serde_json::from_str::<Quality>("900").unwrap(),
            Quality::MAX
        );
    }

    #[test]
    fn percent_and_permille_round_trip_as_bare_numbers() {
        assert_eq!(serde_json::to_string(&Percent(150)).unwrap(), "150");
        assert_eq!(
            serde_json::from_str::<Percent>("150").unwrap(),
            Percent(150)
        );
        assert_eq!(serde_json::to_string(&Permille::WHOLE).unwrap(), "1000");
        assert_eq!(
            serde_json::from_str::<Permille>("250").unwrap(),
            Permille(250)
        );
    }
}
