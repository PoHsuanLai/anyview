//! The words a photo's exposure, camera and date are shown as. Pure: integers in, text out.

use super::{Exposure, Ratio, SignedRatio};

/// The model, with the maker in front unless the model already starts with it (cameras often
/// write `Canon` and `Canon EOS R5`).
pub(super) fn camera(make: Option<&str>, model: Option<&str>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) if starts_with_word(model, make) => Some(model.to_owned()),
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (Some(only), None) | (None, Some(only)) => Some(only.to_owned()),
        (None, None) => None,
    }
}

/// Whether `text` begins with `word` as a whole word, ignoring case.
fn starts_with_word(text: &str, word: &str) -> bool {
    let mut words = text.split_whitespace();
    words
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case(word))
}

/// `1/200 s`, `2 s` or `0.5 s`: a shutter time.
fn shutter(time: Ratio) -> String {
    let Ratio {
        numerator,
        denominator,
    } = time;
    if numerator == 0 {
        return "0 s".to_owned();
    }
    if numerator < denominator {
        // A fraction of a second reads as `1/n` with n rounded to a whole number.
        let n = (u64::from(denominator) + u64::from(numerator) / 2) / u64::from(numerator);
        return format!("1/{n} s");
    }
    let tenths = u64::from(numerator) * 10 / u64::from(denominator);
    match tenths % 10 {
        0 => format!("{} s", tenths / 10),
        fraction => format!("{}.{fraction} s", tenths / 10),
    }
}

/// `f/2.8`, with one digit after the point and none when it is whole.
fn aperture(f_number: Ratio) -> String {
    let tenths = (u64::from(f_number.numerator) * 10 + u64::from(f_number.denominator) / 2)
        / u64::from(f_number.denominator);
    match tenths % 10 {
        0 => format!("f/{}", tenths / 10),
        fraction => format!("f/{}.{fraction}", tenths / 10),
    }
}

/// `35 mm`, rounded to a whole millimetre.
pub(super) fn focal_length(length: Ratio) -> String {
    let mm = (u64::from(length.numerator) + u64::from(length.denominator) / 2)
        / u64::from(length.denominator);
    format!("{mm} mm")
}

/// The recorded parts of an exposure, joined with ` · `; `None` when nothing was recorded.
pub(super) fn exposure(exposure: &Exposure) -> Option<String> {
    let parts: Vec<String> = [
        exposure.shutter.map(shutter),
        exposure.aperture.map(aperture),
        exposure.iso.map(|iso| format!("ISO {iso}")),
        exposure.focal_length.map(focal_length),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// `2024:05:01 12:30:45` as `2024-05-01 12:30`; anything else comes back as it was.
pub(super) fn taken(raw: &str) -> String {
    let digits = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit());
    let Some((date, time)) = raw.split_once(' ') else {
        return raw.to_owned();
    };
    let date_parts: Vec<&str> = date.split(':').collect();
    let time_parts: Vec<&str> = time.split(':').collect();
    match (date_parts.as_slice(), time_parts.as_slice()) {
        ([year, month, day], [hour, minute, ..])
            if digits(year, 4)
                && digits(month, 2)
                && digits(day, 2)
                && digits(hour, 2)
                && digits(minute, 2) =>
        {
            format!("{year}-{month}-{day} {hour}:{minute}")
        }
        _ => raw.to_owned(),
    }
}

/// `1/200 s · f/2.8`: the shutter and the aperture, the parts that were recorded.
pub(super) fn settings(exposure: &Exposure) -> Option<String> {
    let parts: Vec<String> = [
        exposure.shutter.map(shutter),
        exposure.aperture.map(aperture),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// `+0.7 EV`, `-1 EV`, `0 EV`: an exposure bias in stops, to a tenth.
pub(super) fn bias(bias: SignedRatio) -> String {
    let tenths = (i64::from(bias.numerator).abs() * 10 + i64::from(bias.denominator).abs() / 2)
        / i64::from(bias.denominator).abs();
    let negative = (bias.numerator < 0) != (bias.denominator < 0);
    match (tenths, negative) {
        (0, _) => "0 EV".to_owned(),
        (_, negative) => {
            let sign = if negative { '-' } else { '+' };
            match tenths % 10 {
                0 => format!("{sign}{} EV", tenths / 10),
                fraction => format!("{sign}{}.{fraction} EV", tenths / 10),
            }
        }
    }
}
