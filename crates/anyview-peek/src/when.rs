//! A modification time as a person reads it.

use anyview_core::ModTime;

const NANOS_PER_SECOND: i64 = 1_000_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// `2026-10-02 14:30 UTC`. The time zone is UTC until the platform edge can say the person's own:
/// this crate reads no clock and no zone database, so the pane never guesses one.
pub fn modified_text(modified: ModTime) -> String {
    let seconds = modified.0.div_euclid(NANOS_PER_SECOND);
    let (days, in_day) = (
        seconds.div_euclid(SECONDS_PER_DAY),
        seconds.rem_euclid(SECONDS_PER_DAY),
    );
    let (year, month, day) = civil_from_days(days);
    let (hour, minute) = (in_day / 3600, in_day / 60 % 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

/// The proleptic Gregorian date of a day count from 1970-01-01 (Howard Hinnant's
/// `civil_from_days`): shifted to start years in March, so the leap day ends the year.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_utc_dates() {
        // name, seconds since the epoch, nanoseconds on top, words
        const CASES: &[(&str, i64, i64, &str)] = &[
            ("the epoch", 0, 0, "1970-01-01 00:00 UTC"),
            ("a minute in", 61, 0, "1970-01-01 00:01 UTC"),
            (
                "nanoseconds are dropped",
                59,
                999_999_999,
                "1970-01-01 00:00 UTC",
            ),
            (
                "the last second of a day",
                86_399,
                0,
                "1970-01-01 23:59 UTC",
            ),
            ("a leap day", 951_782_400, 0, "2000-02-29 00:00 UTC"),
            ("the day after it", 951_868_800, 0, "2000-03-01 00:00 UTC"),
            ("this decade", 1_790_951_400, 0, "2026-10-02 14:30 UTC"),
            ("before the epoch", -1, 0, "1969-12-31 23:59 UTC"),
            (
                "far before it",
                -86_400 * 365 * 70,
                0,
                "1900-01-18 00:00 UTC",
            ),
        ];
        for (name, seconds, nanos, want) in CASES {
            let time = ModTime(seconds * NANOS_PER_SECOND + nanos);
            assert_eq!(modified_text(time), *want, "{name}");
        }
    }
}
