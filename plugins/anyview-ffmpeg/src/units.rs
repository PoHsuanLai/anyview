//! The one unit this plugin counts in.

/// A span or a point of a recording, in microseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Micros(pub u64);

impl Micros {
    /// The time `seconds` names, rounded to a microsecond; none when it is negative or not a
    /// number.
    pub fn from_secs_f64(seconds: f64) -> Option<Micros> {
        if seconds.is_finite() && seconds >= 0.0 {
            Some(Micros((seconds * 1_000_000.0).round() as u64))
        } else {
            None
        }
    }

    /// Whole seconds, rounded down.
    pub fn whole_secs(self) -> u64 {
        self.0 / 1_000_000
    }

    /// This much, or nothing when `other` is more.
    pub fn minus(self, other: Micros) -> Micros {
        Micros(self.0.saturating_sub(other.0))
    }

    /// Seconds with six decimals, the way ffmpeg's `-t` reads them.
    pub fn ffmpeg_seconds(self) -> String {
        format!("{}.{:06}", self.0 / 1_000_000, self.0 % 1_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_to_microseconds_and_print_back() {
        assert_eq!(Micros::from_secs_f64(1.232), Some(Micros(1_232_000)));
        assert_eq!(Micros::from_secs_f64(-1.0), None);
        assert_eq!(Micros::from_secs_f64(f64::NAN), None);
        assert_eq!(Micros(1_232_000).ffmpeg_seconds(), "1.232000");
        assert_eq!(Micros(5).ffmpeg_seconds(), "0.000005");
        assert_eq!(Micros(3).minus(Micros(9)), Micros(0));
    }
}
