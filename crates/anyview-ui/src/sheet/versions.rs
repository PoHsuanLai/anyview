//! The kept versions of a file as the Revert To sheet lists them: a name the host can find the
//! version by, when it was kept and how large it is. Held with a `Cow` so a state table can spell
//! a list as a constant.

use anyview_core::ByteLen;
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
    /// The row as the sheet words it: when it was kept (UTC) and its size.
    pub fn label(&self) -> String {
        format!("{} ({})", utc_label(self.saved_at), size_label(self.size))
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

/// `YYYY-MM-DD HH:MM UTC` for `seconds` after the Unix epoch.
pub(crate) fn utc_label(seconds: u64) -> String {
    let (days, in_day) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil_of_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        in_day / 3600,
        in_day % 3600 / 60
    )
}

/// The year, month and day `days` after 1970-01-01 (the proleptic Gregorian calendar, counted
/// from a March year so the leap day is the last of it).
fn civil_of_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let of_era = shifted % 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let day_of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
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
    fn a_moment_is_worded_in_utc() {
        // name, seconds since the epoch, words
        const CASES: &[(&str, u64, &str)] = &[
            ("the epoch", 0, "1970-01-01 00:00 UTC"),
            (
                "a leap day",
                951_782_400 + 3600 + 120,
                "2000-02-29 01:02 UTC",
            ),
            ("the end of a year", 1_767_225_599, "2025-12-31 23:59 UTC"),
            ("a recent day", 1_790_000_000, "2026-09-21 14:13 UTC"),
        ];
        for (name, seconds, words) in CASES {
            assert_eq!(utc_label(*seconds), *words, "{name}");
        }
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
