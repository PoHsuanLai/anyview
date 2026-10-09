//! PDF dates: `D:YYYYMMDDHHmmSSOHH'mm'`, every part after the year optional.

use anyview_core::{FactTime, FactZone};

/// The moment `text` names, or `None` when it is not a PDF date. Lenient the way readers are: the
/// `D:` may be missing, and the month, day and time default to the start of the period.
pub(super) fn parse(text: &str) -> Option<FactTime> {
    let text = text.trim();
    let text = text.strip_prefix("D:").unwrap_or(text);
    let digits = |from: usize, len: usize| -> Option<Option<u32>> {
        match text.get(from..from + len) {
            Some(part) if part.bytes().all(|b| b.is_ascii_digit()) => Some(part.parse().ok()),
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
    // Seconds are read past, then the zone: `Z`, or `+HH'mm'` / `-HH'mm'` (the quotes optional).
    let zone = text.get(12..).map_or(FactZone::Unstated, |rest| {
        let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        zone(rest)
    });
    date.at(
        u8::try_from(hour).ok()?,
        u8::try_from(minute.unwrap_or(0)).ok()?,
        zone,
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
    use anyview_core::FactValue;

    #[test]
    fn pdf_dates_read_with_their_zone() {
        // name, raw, words
        const CASES: &[(&str, &str, Option<&str>)] = &[
            (
                "full with offset",
                "D:20261009140500+02'00'",
                Some("9 Oct 2026 at 14:05 +02:00"),
            ),
            ("utc", "D:20261009140500Z", Some("9 Oct 2026 at 14:05 UTC")),
            (
                "negative half hour",
                "D:20261009140500-03'30'",
                Some("9 Oct 2026 at 14:05 -03:30"),
            ),
            (
                "offset without quotes",
                "D:20261009140500+0100",
                Some("9 Oct 2026 at 14:05 +01:00"),
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
            (
                "no prefix",
                "20261009140500Z",
                Some("9 Oct 2026 at 14:05 UTC"),
            ),
            ("not a date", "yesterday", None),
            ("month 13", "D:20261309", None),
            ("empty", "", None),
        ];
        for (name, raw, want) in CASES {
            let got = parse(raw).map(|time| FactValue::date(time).as_str().to_owned());
            assert_eq!(got.as_deref(), *want, "{name}");
        }
    }
}
