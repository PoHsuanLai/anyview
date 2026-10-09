//! A date or a moment as a fact shows it: `9 Oct 2026 at 14:05`, the way Finder and Preview word
//! it, in no one's locale and always in the person's own time zone.

use super::FactValue;
use crate::source::ModTime;
use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};

const NANOS_PER_SECOND: i64 = 1_000_000_000;
const SECONDS_PER_DAY: i64 = 86_400;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[cfg(feature = "testing")]
static PINNED: std::sync::OnceLock<i16> = std::sync::OnceLock::new();

/// The person's own time zone, which every shown moment is converted to. The system zone is read
/// once per [`LocalZone::system`]; tests and hosts that know better name one with
/// [`LocalZone::fixed`].
#[derive(Debug, Clone)]
pub struct LocalZone(TimeZone);

impl LocalZone {
    /// The time zone the system is set to (the `TZ` variable, `/etc/localtime`, the registry);
    /// UTC where it cannot be told.
    pub fn system() -> Self {
        #[cfg(feature = "testing")]
        if let Some(minutes) = PINNED.get() {
            return LocalZone::fixed(*minutes);
        }
        LocalZone(TimeZone::system())
    }

    /// Makes [`LocalZone::system`] answer a zone that is `minutes` east of UTC for the rest of
    /// the process, so a test's expected text does not depend on the machine it runs on. The first
    /// pin wins; every test of a binary pins the same zone.
    #[cfg(feature = "testing")]
    pub fn pin(minutes: i16) {
        let _ = PINNED.set(minutes);
    }

    /// A zone that is always this many minutes east of UTC (west is negative).
    pub fn fixed(minutes: i16) -> Self {
        let seconds = i32::from(minutes) * 60;
        LocalZone(Offset::from_seconds(seconds).map_or(TimeZone::UTC, TimeZone::fixed))
    }

    /// Seconds east of UTC at the instant `unix` seconds after the epoch.
    fn offset_at(&self, unix: i64) -> i64 {
        Timestamp::from_second(unix).map_or(0, |at| i64::from(self.0.to_offset(at).seconds()))
    }
}

/// The zone a moment's clock reading is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactZone {
    /// The file does not say: a camera's clock is whatever the owner set, which is the person's
    /// own zone as far as anyone can tell. Shown as written.
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

    /// This moment read in `zone` instead of the one it was built with.
    pub fn in_zone(self, zone: FactZone) -> Self {
        FactTime { zone, ..self }
    }

    /// The moment `modified` names, as a UTC reading; [`FactValue::date`] shows it in the person's
    /// own zone.
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

    /// This moment as a clock reading in `zone`; a moment with no zone stated, or no time of day,
    /// is already in it.
    fn localised(self, zone: &LocalZone) -> Self {
        let (Some((hour, minute)), Some(stated)) = (self.clock, self.zone.seconds()) else {
            return self;
        };
        let local = days_from_civil(i64::from(self.year), self.month, self.day) * SECONDS_PER_DAY
            + i64::from(hour) * 3600
            + i64::from(minute) * 60
            - stated;
        let local = local + zone.offset_at(local);
        let (days, in_day) = (
            local.div_euclid(SECONDS_PER_DAY),
            local.rem_euclid(SECONDS_PER_DAY),
        );
        let (year, month, day) = civil_from_days(days);
        FactTime {
            year: i32::try_from(year).unwrap_or(self.year),
            month: u8::try_from(month).unwrap_or(self.month),
            day: u8::try_from(day).unwrap_or(self.day),
            clock: Some((
                u8::try_from(in_day / 3600).unwrap_or(0),
                u8::try_from(in_day / 60 % 60).unwrap_or(0),
            )),
            zone: FactZone::Unstated,
        }
    }

    /// `9 Oct 2026`, then ` at 14:05` when the time of day is known.
    fn text(self) -> String {
        let mut text = format!(
            "{} {} {}",
            self.day,
            MONTHS[usize::from(self.month) - 1],
            self.year
        );
        if let Some((hour, minute)) = self.clock {
            text.push_str(&format!(" at {hour:02}:{minute:02}"));
        }
        text
    }
}

impl FactZone {
    /// Seconds east of UTC, or `None` when the file does not say.
    fn seconds(self) -> Option<i64> {
        match self {
            FactZone::Unstated => None,
            FactZone::Utc => Some(0),
            FactZone::Offset(minutes) => Some(i64::from(minutes) * 60),
        }
    }
}

impl FactValue {
    /// `9 Oct 2026 at 14:05` in the person's own time zone, or `9 Oct 2026` for a date with no
    /// time of day.
    pub fn date(when: FactTime) -> Self {
        FactValue::date_in(when, &LocalZone::system())
    }

    /// [`FactValue::date`] in a named zone.
    pub fn date_in(when: FactTime, zone: &LocalZone) -> Self {
        FactValue::text(when.localised(zone).text())
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

/// The day count from 1970-01-01 of a proleptic Gregorian date (`civil_from_days` backwards).
fn days_from_civil(year: i64, month: u8, day: u8) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_index = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_index + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
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

    fn shown(when: FactTime, minutes: i16) -> String {
        FactValue::date_in(when, &LocalZone::fixed(minutes))
            .as_str()
            .to_owned()
    }

    #[test]
    fn dates_read_as_finder_words_them() {
        let day = FactTime::date(2026, 10, 9).unwrap();
        assert_eq!(shown(day, 540), "9 Oct 2026");
        let moment = day.at(14, 5, FactZone::Unstated).unwrap();
        assert_eq!(shown(moment, 540), "9 Oct 2026 at 14:05");
    }

    #[test]
    fn stated_zones_are_converted_to_the_persons_and_never_named() {
        let day = FactTime::date(2026, 10, 9).unwrap();
        let utc = day.at(0, 0, FactZone::Utc).unwrap();
        assert_eq!(shown(utc, 0), "9 Oct 2026 at 00:00");
        assert_eq!(shown(utc, -480), "8 Oct 2026 at 16:00");
        assert_eq!(shown(utc, 330), "9 Oct 2026 at 05:30");
        let east = day.at(9, 30, FactZone::Offset(330)).unwrap();
        assert_eq!(shown(east, 330), "9 Oct 2026 at 09:30");
        assert_eq!(shown(east, 0), "9 Oct 2026 at 04:00");
        let west = day.at(23, 30, FactZone::Offset(-480)).unwrap();
        assert_eq!(shown(west, 480), "10 Oct 2026 at 15:30");
        let new_year = FactTime::date(2026, 12, 31)
            .unwrap()
            .at(23, 0, FactZone::Utc)
            .unwrap();
        assert_eq!(shown(new_year, 120), "1 Jan 2027 at 01:00");
    }

    #[test]
    fn a_moment_with_no_zone_is_shown_as_written() {
        let when = FactTime::date(2024, 5, 1)
            .unwrap()
            .at(12, 30, FactZone::Unstated)
            .unwrap();
        for minutes in [-480, 0, 540] {
            assert_eq!(shown(when, minutes), "1 May 2024 at 12:30");
        }
    }

    #[test]
    fn civil_days_round_trip() {
        for days in [-800_000, -1, 0, 1, 11_000, 20_000, 800_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(
                days_from_civil(y, u8::try_from(m).unwrap(), u8::try_from(d).unwrap()),
                days
            );
        }
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
    fn file_times_are_shown_in_the_persons_zone() {
        // name, seconds since the epoch, minutes east, words
        const CASES: &[(&str, i64, i16, &str)] = &[
            ("the epoch", 0, 0, "1 Jan 1970 at 00:00"),
            ("a leap day", 951_782_400, 0, "29 Feb 2000 at 00:00"),
            ("this decade", 1_790_951_400, 0, "2 Oct 2026 at 14:30"),
            (
                "east of Greenwich",
                1_790_951_400,
                480,
                "2 Oct 2026 at 22:30",
            ),
            (
                "west of Greenwich",
                1_790_951_400,
                -900,
                "1 Oct 2026 at 23:30",
            ),
            ("before the epoch", -1, 0, "31 Dec 1969 at 23:59"),
        ];
        for (name, seconds, minutes, want) in CASES {
            let time = FactTime::from_mod_time(ModTime(seconds * NANOS_PER_SECOND));
            assert_eq!(shown(time, *minutes), *want, "{name}");
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
