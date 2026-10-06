//! What a stream of samples is made of.

use std::num::{NonZeroU16, NonZeroU32};

/// The shape of interleaved 32-bit float samples: how many frames a second and how many channels
/// in each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamFormat {
    /// Frames a second.
    pub rate: NonZeroU32,
    /// Samples in a frame.
    pub channels: NonZeroU16,
}

impl StreamFormat {
    /// A format of `rate` frames a second and `channels` channels, or `None` when either is zero.
    pub fn new(rate: u32, channels: u16) -> Option<StreamFormat> {
        Some(StreamFormat {
            rate: NonZeroU32::new(rate)?,
            channels: NonZeroU16::new(channels)?,
        })
    }

    /// Samples in a frame, as a count.
    pub(super) fn width(self) -> usize {
        usize::from(self.channels.get())
    }

    /// Frames a second, as a count.
    pub(super) fn frames_a_second(self) -> u64 {
        u64::from(self.rate.get())
    }

    /// How many whole frames `micros` microseconds of sound take, rounded down.
    pub(super) fn frames_in(self, micros: u64) -> u64 {
        let frames = u128::from(micros) * u128::from(self.rate.get()) / 1_000_000;
        u64::try_from(frames).unwrap_or(u64::MAX)
    }

    /// How many microseconds `frames` frames last, rounded down.
    pub(super) fn micros_in(self, frames: u64) -> u64 {
        let micros = u128::from(frames) * 1_000_000 / u128::from(self.rate.get());
        u64::try_from(micros).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_microseconds_convert_both_ways() {
        let format = StreamFormat::new(44_100, 2).unwrap();
        const CASES: &[(&str, u64, u64)] = &[
            ("none", 0, 0),
            ("one second", 1_000_000, 44_100),
            ("a tenth", 100_000, 4_410),
            ("rounded down", 22, 0),
        ];
        for (name, micros, frames) in CASES {
            assert_eq!(format.frames_in(*micros), *frames, "{name}");
        }
        assert_eq!(format.micros_in(44_100), 1_000_000);
        assert_eq!(format.micros_in(4_410), 100_000);
    }

    #[test]
    fn a_format_with_no_rate_or_no_channels_is_none() {
        assert_eq!(StreamFormat::new(0, 2), None);
        assert_eq!(StreamFormat::new(44_100, 0), None);
    }
}
