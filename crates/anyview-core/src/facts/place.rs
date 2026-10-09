//! Where a photo was taken, as a fact shows it. Nothing here looks a place up: a coordinate is
//! only worded, never sent anywhere.

use super::FactValue;

const MICRO: i32 = 1_000_000;

/// A latitude and longitude in millionths of a degree. Built only from a real place on Earth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coordinate {
    latitude: i32,
    longitude: i32,
}

impl Coordinate {
    /// The place at these millionths of a degree, north and east positive; `None` when the
    /// latitude is beyond a pole or the longitude beyond the date line.
    pub fn new(latitude_micro: i32, longitude_micro: i32) -> Option<Self> {
        let valid = latitude_micro.abs() <= 90 * MICRO && longitude_micro.abs() <= 180 * MICRO;
        valid.then_some(Coordinate {
            latitude: latitude_micro,
            longitude: longitude_micro,
        })
    }

    /// `37.7749° N, 122.4194° W`: four decimals, about eleven metres.
    fn text(self) -> String {
        let part = |micro: i32, positive: char, negative: char| {
            let ten_thousandths = (i64::from(micro).abs() + 50) / 100;
            format!(
                "{}.{:04}° {}",
                ten_thousandths / 10_000,
                ten_thousandths % 10_000,
                if micro < 0 { negative } else { positive }
            )
        };
        format!(
            "{}, {}",
            part(self.latitude, 'N', 'S'),
            part(self.longitude, 'E', 'W')
        )
    }
}

impl FactValue {
    /// `37.7749° N, 122.4194° W`.
    pub fn coordinate(place: Coordinate) -> Self {
        FactValue::text(place.text())
    }

    /// `12 m`, or `3 m below sea level`.
    pub fn altitude(metres: i32) -> Self {
        FactValue::text(if metres < 0 {
            format!("{} m below sea level", metres.unsigned_abs())
        } else {
            format!("{metres} m")
        })
    }

    /// `1.7`, `2.0.1`: the numbers of a format's version, joined by points.
    pub fn version(parts: &[u32]) -> Self {
        FactValue::text(
            parts
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("."),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_read_with_hemispheres() {
        // name, latitude, longitude, words
        const CASES: &[(&str, i32, i32, &str)] = &[
            (
                "san francisco",
                37_774_900,
                -122_419_400,
                "37.7749° N, 122.4194° W",
            ),
            (
                "sydney",
                -33_868_800,
                151_209_300,
                "33.8688° S, 151.2093° E",
            ),
            ("the origin", 0, 0, "0.0000° N, 0.0000° E"),
            (
                "rounds to the fourth",
                1_234_560,
                6_543_219,
                "1.2346° N, 6.5432° E",
            ),
        ];
        for (name, lat, lon, want) in CASES {
            let place = Coordinate::new(*lat, *lon).unwrap();
            assert_eq!(FactValue::coordinate(place).as_str(), *want, "{name}");
        }
        assert_eq!(Coordinate::new(90_000_001, 0), None);
        assert_eq!(Coordinate::new(0, -180_000_001), None);
        assert!(Coordinate::new(-90_000_000, 180_000_000).is_some());
    }

    #[test]
    fn altitudes_and_versions_read_naturally() {
        assert_eq!(FactValue::altitude(12).as_str(), "12 m");
        assert_eq!(FactValue::altitude(0).as_str(), "0 m");
        assert_eq!(FactValue::altitude(-3).as_str(), "3 m below sea level");
        assert_eq!(FactValue::version(&[1, 7]).as_str(), "1.7");
        assert_eq!(FactValue::version(&[2]).as_str(), "2");
        assert_eq!(FactValue::version(&[]).as_str(), "");
    }
}
