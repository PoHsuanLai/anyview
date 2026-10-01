//! `FileStamp`: what changing on disk looks like, so a loaded file can tell it was edited.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A size in bytes.
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
pub struct ByteLen(pub u64);

/// A modification time as signed nanoseconds since the Unix epoch. An `i64` spans the years
/// 1677 to 2262, which holds every time a file system reports; a time outside that span is
/// clamped to its end by [`ModTime::from_system_time`].
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
pub struct ModTime(pub i64);

impl ModTime {
    /// The instant `time` names, in nanoseconds since the epoch, clamped to the `i64` span. A
    /// pure conversion: nothing here reads the clock.
    pub fn from_system_time(time: SystemTime) -> ModTime {
        let nanos = |span: Duration| i64::try_from(span.as_nanos()).unwrap_or(i64::MAX);
        match time.duration_since(UNIX_EPOCH) {
            Ok(after) => ModTime(nanos(after)),
            Err(before) => ModTime(nanos(before.duration()).saturating_neg()),
        }
    }
}

/// A file's size and modification time when it was read. Two stamps differ when the file was
/// edited; it also keys caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FileStamp {
    /// Size in bytes.
    pub len: ByteLen,
    /// Last modification.
    pub modified: ModTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_time_becomes_signed_nanoseconds() {
        const CASES: &[(&str, i64, i64)] = &[
            ("epoch", 0, 0),
            ("after", 1_500, 1_500),
            ("before", -2_000, -2_000),
        ];
        for (name, nanos, want) in CASES {
            let time = if *nanos >= 0 {
                UNIX_EPOCH + Duration::from_nanos(nanos.unsigned_abs())
            } else {
                UNIX_EPOCH - Duration::from_nanos(nanos.unsigned_abs())
            };
            assert_eq!(ModTime::from_system_time(time), ModTime(*want), "{name}");
        }
    }

    #[test]
    fn a_time_past_the_span_clamps() {
        let far = UNIX_EPOCH + Duration::from_secs(u64::MAX / 4);
        assert_eq!(ModTime::from_system_time(far), ModTime(i64::MAX));
    }

    #[test]
    fn a_stamp_round_trips_and_differs_when_the_file_changes() {
        let stamp = FileStamp {
            len: ByteLen(10),
            modified: ModTime(5),
        };
        let json = serde_json::to_string(&stamp).unwrap();
        assert_eq!(json, r#"{"len":10,"modified":5}"#);
        assert_eq!(serde_json::from_str::<FileStamp>(&json).unwrap(), stamp);
        assert_ne!(
            stamp,
            FileStamp {
                len: ByteLen(11),
                ..stamp
            }
        );
        assert_ne!(
            stamp,
            FileStamp {
                modified: ModTime(6),
                ..stamp
            }
        );
    }
}
