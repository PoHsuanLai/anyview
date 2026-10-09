//! The text a fact shows, formatted for a person.

use crate::source::ByteLen;
use crate::units::{Bitrate, MediaLength, PageCount, PixelSize};

/// The text shown for a fact, already formatted for a person. A producer builds one from its typed
/// value with the constructors here, so a pane never formats a number itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FactValue(String);

impl FactValue {
    /// A value that is already text (a title, a codec name, a date the caller formatted).
    pub fn text(text: impl Into<String>) -> Self {
        FactValue(text.into())
    }

    /// `1920 × 1080`.
    pub fn dimensions(size: PixelSize) -> Self {
        FactValue(format!("{} × {}", size.width.0, size.height.0))
    }

    /// `1 page` or `12 pages`.
    pub fn pages(count: PageCount) -> Self {
        let n = count.get();
        FactValue(format!("{n} {}", if n == 1 { "page" } else { "pages" }))
    }

    /// `3:07`, or `1:02:03` from one hour; seconds are rounded down.
    pub fn duration(length: MediaLength) -> Self {
        let secs = length.0.0 / 1_000_000;
        let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
        FactValue(if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        })
    }

    /// `192 kbit/s`.
    pub fn bitrate(rate: Bitrate) -> Self {
        FactValue(format!("{} kbit/s", rate.kbps()))
    }

    /// `44.1 kHz`, or `48 kHz`: samples a second in kilohertz, trailing zeros dropped.
    pub fn sample_rate(hertz: u32) -> Self {
        let tenths = (u64::from(hertz) + 50) / 100;
        let (whole, tenth) = (tenths / 10, tenths % 10);
        FactValue(if tenth == 0 {
            format!("{whole} kHz")
        } else {
            format!("{whole}.{tenth} kHz")
        })
    }

    /// `mono`, `stereo`, or `6 channels`.
    pub fn channels(count: u16) -> Self {
        FactValue(match count {
            1 => "mono".to_owned(),
            2 => "stereo".to_owned(),
            n => format!("{n} channels"),
        })
    }

    /// A size in decimal units with one digit after the point, rounded down: `412 B`, `1.5 KB`,
    /// `12.3 MB`.
    pub fn size(len: ByteLen) -> Self {
        const UNITS: &[(&str, u128)] = &[
            ("TB", 1_000_000_000_000),
            ("GB", 1_000_000_000),
            ("MB", 1_000_000),
            ("KB", 1_000),
        ];
        let bytes = u128::from(len.0);
        let text = UNITS
            .iter()
            .find(|(_, unit)| bytes >= *unit)
            .map(|(name, unit)| {
                let tenths = bytes * 10 / unit;
                format!("{}.{} {name}", tenths / 10, tenths % 10)
            })
            .unwrap_or_else(|| format!("{bytes} B"));
        FactValue(text)
    }

    /// A size and its exact bytes: `3.2 MB (3,214,880 bytes)`; a size under a kilobyte is just
    /// `412 B`.
    pub fn size_exact(len: ByteLen) -> Self {
        let short = FactValue::size(len);
        if len.0 < 1_000 {
            return short;
        }
        let digits = len.0.to_string();
        let mut grouped = String::new();
        for (at, digit) in digits.chars().enumerate() {
            if at > 0 && (digits.len() - at).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(digit);
        }
        FactValue(format!("{} ({grouped} bytes)", short.0))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{MediaTime, PixelLen};

    #[test]
    fn sizes_are_decimal_with_one_digit() {
        const CASES: &[(&str, u64, &str)] = &[
            ("zero", 0, "0 B"),
            ("bytes", 999, "999 B"),
            ("one kilobyte", 1_000, "1.0 KB"),
            ("kilobytes round down", 1_599, "1.5 KB"),
            ("just under a megabyte", 999_999, "999.9 KB"),
            ("one megabyte", 1_000_000, "1.0 MB"),
            ("megabytes", 12_345_678, "12.3 MB"),
            ("gigabytes", 4_000_000_000, "4.0 GB"),
            ("terabytes", 2_500_000_000_000, "2.5 TB"),
            ("the largest", u64::MAX, "18446744.0 TB"),
        ];
        for (name, bytes, want) in CASES {
            assert_eq!(FactValue::size(ByteLen(*bytes)).as_str(), *want, "{name}");
        }
    }

    #[test]
    fn exact_sizes_group_their_digits() {
        const CASES: &[(&str, u64, &str)] = &[
            ("bytes", 412, "412 B"),
            ("a kilobyte", 1_000, "1.0 KB (1,000 bytes)"),
            ("megabytes", 3_214_880, "3.2 MB (3,214,880 bytes)"),
            ("a round million", 1_000_000, "1.0 MB (1,000,000 bytes)"),
            (
                "the largest",
                u64::MAX,
                "18446744.0 TB (18,446,744,073,709,551,615 bytes)",
            ),
        ];
        for (name, bytes, want) in CASES {
            assert_eq!(
                FactValue::size_exact(ByteLen(*bytes)).as_str(),
                *want,
                "{name}"
            );
        }
    }

    #[test]
    fn sample_rates_and_channels_read_as_a_person_says_them() {
        const RATES: &[(u32, &str)] = &[
            (44_100, "44.1 kHz"),
            (48_000, "48 kHz"),
            (22_050, "22.1 kHz"),
            (8_000, "8 kHz"),
        ];
        for (hertz, want) in RATES {
            assert_eq!(FactValue::sample_rate(*hertz).as_str(), *want, "{hertz}");
        }
        const CHANNELS: &[(u16, &str)] = &[(1, "mono"), (2, "stereo"), (6, "6 channels")];
        for (count, want) in CHANNELS {
            assert_eq!(FactValue::channels(*count).as_str(), *want, "{count}");
        }
    }

    #[test]
    fn durations_read_as_clock_time() {
        const CASES: &[(&str, u64, &str)] = &[
            ("zero", 0, "0:00"),
            ("under a second", 999_999, "0:00"),
            ("seconds", 7_000_000, "0:07"),
            ("minutes", 187_000_000, "3:07"),
            ("an hour", 3_600_000_000, "1:00:00"),
            ("hours", 3_723_000_000, "1:02:03"),
        ];
        for (name, micros, want) in CASES {
            let length = MediaLength(MediaTime(*micros));
            assert_eq!(FactValue::duration(length).as_str(), *want, "{name}");
        }
    }

    #[test]
    fn dimensions_and_pages_read_naturally() {
        let size = PixelSize {
            width: PixelLen(1920),
            height: PixelLen(1080),
        };
        assert_eq!(FactValue::dimensions(size).as_str(), "1920 × 1080");
        assert_eq!(
            FactValue::pages(PageCount::new(1).unwrap()).as_str(),
            "1 page"
        );
        assert_eq!(
            FactValue::pages(PageCount::new(12).unwrap()).as_str(),
            "12 pages"
        );
    }
}
