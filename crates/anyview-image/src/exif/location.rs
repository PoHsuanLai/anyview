//! Where a photo was taken, from the EXIF GPS block. Read here, shown only by the viewer's own
//! Info panel: see `ExifFacts::location_facts`.

use anyview_core::Coordinate;
use exif::{Exif, In, Tag, Value};

/// The place a photo was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    /// Latitude and longitude.
    pub place: Coordinate,
    /// Whole metres above sea level (negative below it), when the camera recorded them.
    pub altitude: Option<i32>,
}

impl Location {
    /// The location the GPS block of `exif` gives, or `None` when it has no usable latitude and
    /// longitude.
    pub(super) fn read(exif: &Exif) -> Option<Self> {
        let field = |tag| exif.get_field(tag, In::PRIMARY).map(|f| &f.value);
        let latitude = signed(field(Tag::GPSLatitude)?, field(Tag::GPSLatitudeRef)?, "S")?;
        let longitude = signed(field(Tag::GPSLongitude)?, field(Tag::GPSLongitudeRef)?, "W")?;
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
/// `negative` hemisphere's letter.
fn signed(value: &Value, reference: &Value, negative: &str) -> Option<i32> {
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
    let south_or_west = letters
        .first()
        .is_some_and(|letter| letter.eq_ignore_ascii_case(negative.as_bytes()));
    Some(if south_or_west { -total } else { total })
}
