//! Where a photo was taken, from the EXIF GPS block. Read here and nowhere else, so that no value
//! a preview carries (`ExifFacts`, the peek) can hold a place: only the viewer's own Info panel
//! calls [`Location::of_file`], through `picture_facts`.

use anyview_core::{Coordinate, FactLabel, FactValue, Facts};
use exif::{Exif, In, Reader, Tag, Value};
use std::io::Cursor;

/// The place a photo was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    /// Latitude and longitude.
    pub place: Coordinate,
    /// Whole metres above sea level (negative below it), when the camera recorded them.
    pub altitude: Option<i32>,
}

impl Location {
    /// The location in the EXIF block of an image file, or `None` when it has no usable one.
    pub fn of_file(file: &[u8]) -> Option<Self> {
        let exif = Reader::new()
            .read_from_container(&mut Cursor::new(file))
            .ok()?;
        Self::read(&exif)
    }

    /// The latitude and longitude as text, and the altitude: the Location rows of the Info panel.
    /// Nothing here looks the place up.
    pub fn facts(self) -> Facts {
        let facts = Facts::empty().with(FactLabel::Coordinates, FactValue::coordinate(self.place));
        match self.altitude {
            Some(metres) => facts.with(FactLabel::Altitude, FactValue::altitude(metres)),
            None => facts,
        }
    }

    /// The location the GPS block of `exif` gives, or `None` when it has no usable latitude and
    /// longitude, when the receiver says it had no fix (`GPSStatus` is `V`), or when the place is
    /// 0, 0 (what a camera without a fix writes).
    fn read(exif: &Exif) -> Option<Self> {
        let field = |tag| exif.get_field(tag, In::PRIMARY).map(|f| &f.value);
        if let Some(Value::Ascii(status)) = field(Tag::GPSStatus)
            && status
                .first()
                .is_some_and(|s| s.first().is_some_and(|c| c.eq_ignore_ascii_case(&b'V')))
        {
            return None;
        }
        let latitude = signed(
            field(Tag::GPSLatitude)?,
            field(Tag::GPSLatitudeRef)?,
            "N",
            "S",
        )?;
        let longitude = signed(
            field(Tag::GPSLongitude)?,
            field(Tag::GPSLongitudeRef)?,
            "E",
            "W",
        )?;
        if latitude == 0 && longitude == 0 {
            return None;
        }
        let altitude = field(Tag::GPSAltitude).and_then(|value| {
            let Value::Rational(metres) = value else {
                return None;
            };
            let above = metres.first().filter(|m| m.denom != 0)?;
            let whole = i32::try_from(
                (u64::from(above.num) + u64::from(above.denom) / 2) / u64::from(above.denom),
            )
            .ok()?;
            let below = field(Tag::GPSAltitudeRef).and_then(|r| r.get_uint(0)) == Some(1);
            Some(if below { -whole } else { whole })
        });
        Some(Location {
            place: Coordinate::new(latitude, longitude)?,
            altitude,
        })
    }
}

/// Degrees, minutes and seconds as millionths of a degree, negative when `reference` is the
/// `negative` hemisphere's letter; `None` when it is neither that nor the `positive` one (a
/// missing or garbled reference cannot be guessed at).
fn signed(value: &Value, reference: &Value, positive: &str, negative: &str) -> Option<i32> {
    let Value::Rational(parts) = value else {
        return None;
    };
    let [degrees, minutes, seconds] = parts.as_slice() else {
        return None;
    };
    // In integers: each part in billionths of a degree, rounded to millionths at the end.
    let nano = |part: &exif::Rational, per_degree: u64| {
        (part.denom != 0).then(|| {
            u128::from(part.num) * 1_000_000_000 / (u128::from(part.denom) * u128::from(per_degree))
        })
    };
    let total = nano(degrees, 1)? + nano(minutes, 60)? + nano(seconds, 3600)?;
    let total = i32::try_from((total + 500) / 1000).ok()?;
    let Value::Ascii(letters) = reference else {
        return None;
    };
    let letter = letters.first()?;
    if letter.eq_ignore_ascii_case(negative.as_bytes()) {
        Some(-total)
    } else if letter.eq_ignore_ascii_case(positive.as_bytes()) {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
