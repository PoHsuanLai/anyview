//! A date or a moment as a fact shows it: `9 Oct 2026 at 14:05`, the way Finder and Preview word
//! it, in no one's locale.

use super::FactValue;
use crate::source::ModTime;

const NANOS_PER_SECOND: i64 = 1_000_000_000;
const SECONDS_PER_DAY: i64 = 86_400;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The zone a moment's clock reading is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactZone {
    /// The file does not say: a camera's clock is whatever the owner set. Shown with no zone.
    Unstated,
    /// Coordinated Universal Time.
    Utc,
    /// This many minutes east of UTC (west is negative).
    Offset(i16),
}

/// A calendar date, with the time of day when it is known. Built only from a real date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactTime {
    year: i32,
    month: u8,
    day: u8,
    clock: Option<(u8, u8)>,
    zone: FactZone,
}

impl FactTime {
    /// A date with no time of day; `None` when `month` and `day` are not a day of that year.
    pub fn date(year: i32, month: u8, day: u8) -> Option<Self> {
        let valid = (1..=12).contains(&month) && day >= 1 && day <= days_in(year, month);
        valid.then_some(FactTime {
            year,
            month,
            day,
            clock: None,
            zone: FactZone::Unstated,
        })
    }

    /// This date at `hour`:`minute` in `zone`; `None` when that is not a time of day.
    pub fn at(self, hour: u8, minute: u8, zone: FactZone) -> Option<Self> {
        (hour < 24 && minute < 60).then_some(FactTime {
            clock: Some((hour, minute)),
            zone,
            ..self
        })
    }

    /// The moment `modified` names, read in UTC: this crate reads no clock and no zone database,
    /// so the person's own zone is for the edge to say.
    pub fn from_mod_time(modified: ModTime) -> Self {
        let seconds = modified.0.div_euclid(NANOS_PER_SECOND);
        let (days, in_day) = (
            seconds.div_euclid(SECONDS_PER_DAY),
            seconds.rem_euclid(SECONDS_PER_DAY),
        );
        let (year, month, day) = civil_from_days(days);
        FactTime {
            year: i32::try_from(year).unwrap_or(i32::MAX),
            month: u8::try_from(month).unwrap_or(1),
            day: u8::try_from(day).unwrap_or(1),
            clock: Some((
                u8::try_from(in_day / 3600).unwrap_or(0),
                u8::try_from(in_day / 60 % 60).unwrap_or(0),
            )),
            zone: FactZone::Utc,
        }
    }

    /// `2024:05:01 12:30:45` as EXIF writes a capture time; `None` for anything else. The zone is
    /// unstated.
    pub fn parse_exif(raw: &str) -> Option<Self> {
        let (date, time) = raw.trim().split_once(' ')?;
        let mut date = date.split(':');
        let mut time = time.split(':');
        let number = |part: Option<&str>| part?.parse::<u32>().ok();
        let year = i32::try_from(number(date.next())?).ok()?;
        let month = u8::try_from(number(date.next())?).ok()?;
        let day = u8::try_from(number(date.next())?).ok()?;
        let hour = u8::try_from(number(time.next())?).ok()?;
        let minute = u8::try_from(number(time.next())?).ok()?;
        FactTime::date(year, month, day)?.at(hour, minute, FactZone::Unstated)
    }

    /// `9 Oct 2026`, then ` at 14:05` and the zone when they are known.
    fn text(self) -> String {
        let mut text = format!(
            "{} {} {}",
            self.day,
            MONTHS[usize::from(self.month) - 1],
            self.year
        );
        if let Some((hour, minute)) = self.clock {
            text.push_str(&format!(" at {hour:02}:{minute:02}"));
            match self.zone {
                FactZone::Unstated => {}
                FactZone::Utc => text.push_str(" UTC"),
                FactZone::Offset(minutes) => {
                    let sign = if minutes < 0 { '-' } else { '+' };
                    let minutes = minutes.unsigned_abs();
                    text.push_str(&format!(" {sign}{:02}:{:02}", minutes / 60, minutes % 60));
                }
            }
        }
        text
    }
}

impl FactValue {
    /// `9 Oct 2026 at 14:05`, or `9 Oct 2026` for a date with no time of day.
    pub fn date(when: FactTime) -> Self {
        FactValue::text(when.text())
    }
}

fn days_in(year: i32, month: u8) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
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
    fn dates_read_as_finder_words_them() {
        let day = FactTime::date(2026, 10, 9).unwrap();
        assert_eq!(FactValue::date(day).as_str(), "9 Oct 2026");
        let moment = day.at(14, 5, FactZone::Unstated).unwrap();
        assert_eq!(FactValue::date(moment).as_str(), "9 Oct 2026 at 14:05");
        let utc = day.at(0, 0, FactZone::Utc).unwrap();
        assert_eq!(FactValue::date(utc).as_str(), "9 Oct 2026 at 00:00 UTC");
        let east = day.at(9, 30, FactZone::Offset(330)).unwrap();
        assert_eq!(FactValue::date(east).as_str(), "9 Oct 2026 at 09:30 +05:30");
        let west = day.at(9, 30, FactZone::Offset(-480)).unwrap();
        assert_eq!(FactValue::date(west).as_str(), "9 Oct 2026 at 09:30 -08:00");
    }

    #[test]
    fn impossible_dates_are_refused() {
        assert_eq!(FactTime::date(2026, 2, 29), None);
        assert!(FactTime::date(2024, 2, 29).is_some());
        assert_eq!(FactTime::date(2100, 2, 29), None);
        assert!(FactTime::date(2000, 2, 29).is_some());
        assert_eq!(FactTime::date(2026, 13, 1), None);
        assert_eq!(FactTime::date(2026, 4, 31), None);
        assert_eq!(FactTime::date(2026, 4, 0), None);
        let day = FactTime::date(2026, 4, 1).unwrap();
        assert_eq!(day.at(24, 0, FactZone::Utc), None);
        assert_eq!(day.at(0, 60, FactZone::Utc), None);
    }

    #[test]
    fn file_times_are_read_in_utc() {
        // name, seconds since the epoch, words
        const CASES: &[(&str, i64, &str)] = &[
            ("the epoch", 0, "1 Jan 1970 at 00:00 UTC"),
            ("a leap day", 951_782_400, "29 Feb 2000 at 00:00 UTC"),
            ("this decade", 1_790_951_400, "2 Oct 2026 at 14:30 UTC"),
            ("before the epoch", -1, "31 Dec 1969 at 23:59 UTC"),
        ];
        for (name, seconds, want) in CASES {
            let time = FactTime::from_mod_time(ModTime(seconds * NANOS_PER_SECOND));
            assert_eq!(FactValue::date(time).as_str(), *want, "{name}");
        }
    }

    #[test]
    fn exif_times_parse_or_are_refused() {
        let got = FactTime::parse_exif("2024:05:01 12:30:45").unwrap();
        assert_eq!(FactValue::date(got).as_str(), "1 May 2024 at 12:30");
        for bad in [
            "",
            "2024-05-01 12:30:45",
            "0000:00:00 00:00:00",
            "2024:05:01",
        ] {
            assert_eq!(FactTime::parse_exif(bad), None, "{bad:?}");
        }
    }
}
