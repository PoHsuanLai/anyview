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

/// How fast a recording plays, in thousandths of its own speed: 1000 is the recording's pace.
/// Construction clamps into the range a person can follow, a quarter to four times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Speed(u32);

impl Speed {
    /// The recording's own pace.
    pub const NORMAL: Speed = Speed(1000);
    /// The slowest speed.
    pub const MIN: Speed = Speed(250);
    /// The fastest speed.
    pub const MAX: Speed = Speed(4000);
    /// The speeds the viewer offers, slowest first; `NORMAL` is among them.
    pub const PRESETS: &'static [Speed] = &[
        Speed(500),
        Speed(750),
        Speed(1000),
        Speed(1250),
        Speed(1500),
        Speed(2000),
    ];

    /// `thousandths` clamped into the range of speeds.
    pub fn from_thousandths(thousandths: u32) -> Self {
        Speed(thousandths.clamp(Self::MIN.0, Self::MAX.0))
    }

    /// The speed in thousandths of the recording's own.
    pub fn thousandths(self) -> u32 {
        self.0
    }
}

impl Default for Speed {
    fn default() -> Self {
        Speed::NORMAL
    }
}

/// A chapter of a recording, counted from the first, which is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChapterIndex(pub u32);

/// A stretch of a recording: from `start` to `end`, or to the end of the recording when `end` is
/// `None`. The constructor refuses an end that is not after the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeRange {
    start: MediaTime,
    end: Option<MediaTime>,
}

impl TimeRange {
    /// All of a recording.
    pub const WHOLE: TimeRange = TimeRange {
        start: MediaTime(0),
        end: None,
    };

    /// From `start` to `end`, or to the end of the recording when `end` is `None`.
    pub fn new(start: MediaTime, end: Option<MediaTime>) -> Result<Self, crate::CoreError> {
        match end {
            Some(end) if end <= start => Err(crate::CoreError::TimeRangeEmpty {
                start: start.0,
                end: end.0,
            }),
            Some(_) | None => Ok(TimeRange { start, end }),
        }
    }

    /// Where the stretch begins.
    pub fn start(self) -> MediaTime {
        self.start
    }

    /// Where it ends, or `None` for the end of the recording.
    pub fn end(self) -> Option<MediaTime> {
        self.end
    }
}

impl Default for TimeRange {
    fn default() -> Self {
        TimeRange::WHOLE
    }
}

/// How many kilobits a second a lossy audio encode spends, clamped to what the encoders take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Bitrate(u32);

impl Bitrate {
    /// The least an encode is given, 32 kbit/s.
    pub const MIN: Bitrate = Bitrate(32);
    /// The most, 512 kbit/s.
    pub const MAX: Bitrate = Bitrate(512);

    /// `kbps` clamped into 32 to 512.
    pub fn from_kbps(kbps: u32) -> Self {
        Bitrate(kbps.clamp(Self::MIN.0, Self::MAX.0))
    }

    /// The rate in kilobits a second.
    pub fn kbps(self) -> u32 {
        self.0
    }

    /// The rate in bits a second, which is what an encoder is told.
    pub fn bits_per_second(self) -> u64 {
        u64::from(self.0) * 1000
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

    #[test]
    fn a_speed_clamps_into_a_quarter_to_four_times() {
        const CASES: &[(&str, u32, u32)] = &[
            ("zero rises to the floor", 0, 250),
            ("the floor", 250, 250),
            ("normal", 1000, 1000),
            ("the ceiling", 4000, 4000),
            ("far above", u32::MAX, 4000),
        ];
        for (name, given, want) in CASES {
            assert_eq!(
                Speed::from_thousandths(*given).thousandths(),
                *want,
                "{name}"
            );
        }
        assert!(Speed::PRESETS.contains(&Speed::NORMAL));
        assert!(Speed::PRESETS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_time_range_ends_after_it_starts() {
        let at = MediaTime::from_secs;
        const CASES: &[(&str, u64, Option<u64>, bool)] = &[
            ("a part", 2, Some(5), true),
            ("to the end", 2, None, true),
            ("empty", 5, Some(5), false),
            ("backwards", 5, Some(2), false),
        ];
        for (name, start, end, ok) in CASES {
            let made = TimeRange::new(at(*start), end.map(at));
            assert_eq!(made.is_ok(), *ok, "{name}");
        }
        assert_eq!(TimeRange::default(), TimeRange::WHOLE);
        assert_eq!(TimeRange::WHOLE.start(), MediaTime(0));
        assert_eq!(TimeRange::WHOLE.end(), None);
    }

    #[test]
    fn a_bitrate_clamps_and_says_its_bits() {
        assert_eq!(Bitrate::from_kbps(0), Bitrate::MIN);
        assert_eq!(Bitrate::from_kbps(9999), Bitrate::MAX);
        assert_eq!(Bitrate::from_kbps(192).kbps(), 192);
        assert_eq!(Bitrate::from_kbps(192).bits_per_second(), 192_000);
    }
}
