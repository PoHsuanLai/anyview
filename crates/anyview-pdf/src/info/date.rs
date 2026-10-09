//! PDF dates: `D:YYYYMMDDHHmmSSOHH'mm'`, every part after the year optional. Writers that got it
//! wrong write ISO 8601 (`D:2026-10-09`), and an XMP packet always does: both are read too.

use anyview_core::{FactTime, FactZone};

/// The moment `text` names, or `None` when it is not a PDF date. Lenient the way readers are: the
/// `D:` may be missing, and the month, day and time default to the start of the period.
pub(super) fn parse(text: &str) -> Option<FactTime> {
    let text = text.trim();
    let text = text.strip_prefix("D:").unwrap_or(text);
    if text.as_bytes().get(4) == Some(&b'-') {
        return iso(text);
    }
    let digits = |from: usize, len: usize| -> Option<Option<u32>> {
        match text.get(from..from + len) {
            Some(part) if part.bytes().all(|b| b.is_ascii_digit()) => Some(part.parse().ok()),
            // A part that starts with the zone (`Z`, `+`, `-`) means the digits ended before it.
            Some(part) if !part.as_bytes()[0].is_ascii_digit() => Some(None),
            Some(_) => None,
            None => Some(None),
        }
    };
    let year = i32::try_from(digits(0, 4)??).ok()?;
    let month = digits(4, 2)?;
    let day = digits(6, 2)?;
    let hour = digits(8, 2)?;
    let minute = digits(10, 2)?;
    let date = FactTime::date(
        year,
        u8::try_from(month.unwrap_or(1)).ok()?,
        u8::try_from(day.unwrap_or(1)).ok()?,
    )?;
    let Some(hour) = hour else {
        return Some(date);
    };
    // The seconds are read past, then the zone: `Z`, or `+HH'mm'` / `-HH'mm'` (the quotes
    // optional). It follows whatever digits there are, so `D:2026100914Z` has its zone too.
    let zone = zone(text.trim_start_matches(|c: char| c.is_ascii_digit()));
    date.at(
        u8::try_from(hour).ok()?,
        u8::try_from(minute.unwrap_or(0)).ok()?,
        zone,
    )
}

/// `2026-10-09`, `2026-10-09T14:05`, `2026-10-09T14:05:30.5+02:00`, `2026-10-09 14:05Z`: ISO 8601
/// with every part after the year optional.
fn iso(text: &str) -> Option<FactTime> {
    let number = |part: Option<&str>, len: usize| -> Option<Option<u32>> {
        match part {
            None => Some(None),
            Some(part) if part.len() == len && part.bytes().all(|b| b.is_ascii_digit()) => {
                Some(part.parse().ok())
            }
            Some(_) => None,
        }
    };
    let (date, time) = match text.split_once(['T', 't', ' ']) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let year = i32::try_from(number(parts.next(), 4)??).ok()?;
    let month = number(parts.next(), 2)?.unwrap_or(1);
    let day = number(parts.next(), 2)?.unwrap_or(1);
    let day = FactTime::date(year, u8::try_from(month).ok()?, u8::try_from(day).ok()?)?;
    let Some(time) = time else {
        return Some(day);
    };
    let clock_end = time.find(['Z', 'z', '+', '-']).unwrap_or(time.len());
    let (clock, rest) = time.split_at(clock_end);
    let clock = clock.split('.').next().unwrap_or("");
    let mut clock = clock.split(':');
    let hour = number(clock.next(), 2)??;
    let minute = number(clock.next(), 2)?.unwrap_or(0);
    day.at(
        u8::try_from(hour).ok()?,
        u8::try_from(minute).ok()?,
        zone(rest),
    )
}

fn zone(rest: &str) -> FactZone {
    let mut chars = rest.chars();
    let sign = match chars.next() {
        Some('Z' | 'z') => return FactZone::Utc,
        Some('+') => 1_i16,
        Some('-') => -1,
        _ => return FactZone::Unstated,
    };
    let groups: Vec<&str> = chars
        .as_str()
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .collect();
    // `+HH'mm'`, or `+HHmm` with no quotes, or `+HH` alone.
    let (hours, minutes) = match groups.as_slice() {
        [one] if one.len() == 4 => (one[..2].parse::<i16>(), one[2..].parse::<i16>()),
        [hours] => (hours.parse::<i16>(), Ok(0)),
        [hours, minutes, ..] => (hours.parse::<i16>(), minutes.parse::<i16>()),
        [] => return FactZone::Unstated,
    };
    match (hours, minutes) {
        (Ok(hours), Ok(minutes)) if hours < 24 && minutes < 60 => {
            FactZone::Offset(sign * (hours * 60 + minutes))
        }
        _ => FactZone::Unstated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FactValue, LocalZone};

    #[test]
    fn pdf_dates_read_with_their_zone() {
        // name, raw, words
        const CASES: &[(&str, &str, Option<&str>)] = &[
            (
                "full with offset",
                "D:20261009140500+02'00'",
                Some("9 Oct 2026 at 12:05"),
            ),
            ("utc", "D:20261009140500Z", Some("9 Oct 2026 at 14:05")),
            (
                "negative half hour",
                "D:20261009140500-03'30'",
                Some("9 Oct 2026 at 17:35"),
            ),
            (
                "offset without quotes",
                "D:20261009140500+0100",
                Some("9 Oct 2026 at 13:05"),
            ),
            ("no zone", "D:20261009140500", Some("9 Oct 2026 at 14:05")),
            (
                "minutes only",
                "D:202610091405",
                Some("9 Oct 2026 at 14:05"),
            ),
            ("date only", "D:20261009", Some("9 Oct 2026")),
            ("year and month", "D:202610", Some("1 Oct 2026")),
            ("year", "D:2026", Some("1 Jan 2026")),
            ("no prefix", "20261009140500Z", Some("9 Oct 2026 at 14:05")),
            ("hour then Z", "D:2026100914Z", Some("9 Oct 2026 at 14:00")),
            (
                "hour then offset",
                "D:2026100914+02'00'",
                Some("9 Oct 2026 at 12:00"),
            ),
            ("iso date", "D:2026-10-09", Some("9 Oct 2026")),
            ("iso month", "2026-10", Some("1 Oct 2026")),
            ("iso year", "2026", Some("1 Jan 2026")),
            (
                "iso to the minute, no zone",
                "2026-10-09T14:05",
                Some("9 Oct 2026 at 14:05"),
            ),
            (
                "iso with seconds and offset",
                "2026-10-09T14:05:30+02:00",
                Some("9 Oct 2026 at 12:05"),
            ),
            (
                "iso with a space and Z",
                "2026-10-09 14:05:30Z",
                Some("9 Oct 2026 at 14:05"),
            ),
            (
                "iso with fractional seconds",
                "2026-10-09T14:05:30.25Z",
                Some("9 Oct 2026 at 14:05"),
            ),
            ("iso month 13", "2026-13-09", None),
            ("not a date", "yesterday", None),
            ("month 13", "D:20261309", None),
            ("empty", "", None),
        ];
        for (name, raw, want) in CASES {
            let zone = LocalZone::fixed(0);
            let got = parse(raw).map(|time| FactValue::date_in(time, &zone).as_str().to_owned());
            assert_eq!(got.as_deref(), *want, "{name}");
        }
    }
}
