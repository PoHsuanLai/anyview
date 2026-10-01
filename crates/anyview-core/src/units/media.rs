//! Media time: a position, a length and a volume.

use super::ratio::Percent;

/// A position in a recording, in microseconds from its start.
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
pub struct MediaTime(pub u64);

impl MediaTime {
    /// A position `millis` milliseconds in, saturating at the largest representable time.
    pub const fn from_millis(millis: u64) -> Self {
        MediaTime(millis.saturating_mul(1_000))
    }

    /// A position `secs` seconds in, saturating at the largest representable time.
    pub const fn from_secs(secs: u64) -> Self {
        MediaTime(secs.saturating_mul(1_000_000))
    }

    /// The position in whole milliseconds, rounded down.
    pub fn as_millis(self) -> u64 {
        self.0 / 1_000
    }
}

/// How long a recording runs.
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
pub struct MediaLength(pub MediaTime);

impl MediaLength {
    /// `at`, or the end of the recording when it is past it.
    pub fn clamp(self, at: MediaTime) -> MediaTime {
        at.min(self.0)
    }
}

/// Output level in percent: 0 is silent, 100 the recording's own level, 150 the loudest the player
/// amplifies to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(from = "Percent", into = "Percent")]
pub struct Volume(Percent);

impl Volume {
    /// The loudest volume.
    pub const MAX: Volume = Volume(Percent(150));
    /// No sound.
    pub const SILENT: Volume = Volume(Percent(0));
    /// The recording's own level.
    pub const FULL: Volume = Volume(Percent(100));

    /// `percent` clamped to at most 150. A stored volume above that loads as 150.
    pub fn clamped(percent: Percent) -> Self {
        Volume(Percent(percent.0.min(Self::MAX.0.0)))
    }

    /// The volume as a percentage, always 0 to 150.
    pub fn percent(self) -> Percent {
        self.0
    }
}

impl From<Percent> for Volume {
    fn from(percent: Percent) -> Self {
        Volume::clamped(percent)
    }
}

impl From<Volume> for Percent {
    fn from(volume: Volume) -> Self {
        volume.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_clamps_at_one_hundred_fifty() {
        const CASES: &[(&str, u16, u16)] = &[
            ("silent", 0, 0),
            ("full", 100, 100),
            ("boost", 150, 150),
            ("above the limit", 151, 150),
            ("far above", u16::MAX, 150),
        ];
        for (name, given, want) in CASES {
            assert_eq!(
                Volume::clamped(Percent(*given)).percent(),
                Percent(*want),
                "{name}"
            );
        }
    }

    #[test]
    fn media_time_converts_and_saturates() {
        const CASES: &[(&str, MediaTime, u64)] = &[
            ("millis", MediaTime::from_millis(1_500), 1_500_000),
            ("secs", MediaTime::from_secs(2), 2_000_000),
            (
                "millis saturate",
                MediaTime::from_millis(u64::MAX),
                u64::MAX,
            ),
            ("secs saturate", MediaTime::from_secs(u64::MAX), u64::MAX),
        ];
        for (name, time, micros) in CASES {
            assert_eq!(time.0, *micros, "{name}");
        }
        assert_eq!(MediaTime(2_999_999).as_millis(), 2_999);
    }

    #[test]
    fn a_length_clamps_a_position_to_the_end() {
        let length = MediaLength(MediaTime::from_secs(10));
        assert_eq!(
            length.clamp(MediaTime::from_secs(4)),
            MediaTime::from_secs(4)
        );
        assert_eq!(
            length.clamp(MediaTime::from_secs(11)),
            MediaTime::from_secs(10)
        );
    }

    #[test]
    fn media_units_round_trip() {
        let volume = Volume::clamped(Percent(120));
        let json = serde_json::to_string(&volume).unwrap();
        assert_eq!(json, "120");
        assert_eq!(serde_json::from_str::<Volume>(&json).unwrap(), volume);
        assert_eq!(serde_json::from_str::<Volume>("400").unwrap(), Volume::MAX);
        let time = MediaTime(123);
        assert_eq!(
            serde_json::from_str::<MediaTime>(&serde_json::to_string(&time).unwrap()).unwrap(),
            time
        );
        let length = MediaLength(time);
        assert_eq!(
            serde_json::from_str::<MediaLength>(&serde_json::to_string(&length).unwrap()).unwrap(),
            length
        );
    }
}
