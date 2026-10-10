//! The kept versions of a file as the Revert To sheet lists them: a name the host can find the
//! version by, when it was kept and how large it is. Held with a `Cow` so a state table can spell
//! a list as a constant.

use anyview_core::{ByteLen, FactTime, FactValue, LocalZone, ModTime};
use std::borrow::Cow;

/// Names one kept version to the host that keeps them. The window never reads it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionKey(Cow<'static, str>);

impl VersionKey {
    /// A key the host made, owned.
    pub fn new(key: impl Into<String>) -> Self {
        VersionKey(Cow::Owned(key.into()))
    }

    /// A literal, held without a copy.
    pub const fn from_static(key: &'static str) -> Self {
        VersionKey(Cow::Borrowed(key))
    }

    /// The key's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One kept version of the open file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionRow {
    /// What the host finds it by.
    pub key: VersionKey,
    /// When it was kept, in seconds since the Unix epoch.
    pub saved_at: u64,
    /// How large it is.
    pub size: ByteLen,
}

impl VersionRow {
    /// The row as the sheet words it: when it was kept (local time) and its size.
    pub fn label(&self) -> String {
        format!("{} ({})", when_label(self.saved_at), size_label(self.size))
    }
}

/// The versions the sheet lists, newest first: never empty once a sheet holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionList(Cow<'static, [VersionRow]>);

impl VersionList {
    /// The list of `rows`, owned; `None` when there are none.
    pub fn new(rows: Vec<VersionRow>) -> Option<Self> {
        (!rows.is_empty()).then_some(VersionList(Cow::Owned(rows)))
    }

    /// A list written out as a constant.
    pub const fn from_static(rows: &'static [VersionRow]) -> Self {
        VersionList(Cow::Borrowed(rows))
    }

    /// The rows, newest first.
    pub fn rows(&self) -> &[VersionRow] {
        &self.0
    }

    /// The key of the newest row, the one the sheet opens on.
    pub fn newest(&self) -> Option<&VersionKey> {
        self.0.first().map(|row| &row.key)
    }

    /// Whether `key` names a row.
    pub fn holds(&self, key: &VersionKey) -> bool {
        self.0.iter().any(|row| row.key == *key)
    }
}

/// `9 Oct 2026 at 14:05` for `seconds` after the Unix epoch, in the person's own time zone: the
/// words every date in the app uses.
pub(crate) fn when_label(seconds: u64) -> String {
    when_label_in(seconds, &LocalZone::system())
}

fn when_label_in(seconds: u64, zone: &LocalZone) -> String {
    let nanos = i64::try_from(seconds).unwrap_or(i64::MAX / 1_000_000_000) * 1_000_000_000;
    FactValue::date_in(FactTime::from_mod_time(ModTime(nanos)), zone)
        .as_str()
        .to_owned()
}

/// A size in the largest of bytes, kB and MB that keeps it above one.
pub(crate) fn size_label(size: ByteLen) -> String {
    let bytes = size.0;
    if bytes < 1000 {
        format!("{bytes} bytes")
    } else if bytes < 1_000_000 {
        format!("{:.1} kB", bytes as f64 / 1000.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_is_worded_in_the_persons_zone() {
        // name, seconds since the epoch, minutes east of UTC, words
        const CASES: &[(&str, u64, i16, &str)] = &[
            ("the epoch", 0, 0, "1 Jan 1970 at 00:00"),
            (
                "a leap day",
                951_782_400 + 3600 + 120,
                0,
                "29 Feb 2000 at 01:02",
            ),
            (
                "the end of a year",
                1_767_225_599,
                0,
                "31 Dec 2025 at 23:59",
            ),
            (
                "east of Greenwich",
                1_767_225_599,
                480,
                "1 Jan 2026 at 07:59",
            ),
            (
                "west of Greenwich",
                1_790_000_000,
                -300,
                "21 Sep 2026 at 09:13",
            ),
        ];
        for (name, seconds, minutes, words) in CASES {
            let zone = LocalZone::fixed(*minutes);
            assert_eq!(when_label_in(*seconds, &zone), *words, "{name}");
        }
        assert!(!when_label(0).contains("UTC"));
    }

    #[test]
    fn a_size_is_worded_in_its_own_unit() {
        // name, bytes, words
        const CASES: &[(&str, u64, &str)] = &[
            ("a few bytes", 12, "12 bytes"),
            ("just under a kilobyte", 999, "999 bytes"),
            ("kilobytes", 1_500, "1.5 kB"),
            ("megabytes", 2_340_000, "2.3 MB"),
        ];
        for (name, bytes, words) in CASES {
            assert_eq!(size_label(ByteLen(*bytes)), *words, "{name}");
        }
    }
}
