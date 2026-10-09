use super::*;
use exif::Rational;

/// One IFD entry: tag, EXIF type (2 ASCII, 5 rational, 1 byte), count, bytes.
type Entry = (u16, u16, u32, Vec<u8>);

fn ascii(tag: u16, text: &str) -> Entry {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    (tag, 2, u32::try_from(bytes.len()).unwrap(), bytes)
}

fn rationals(tag: u16, parts: &[(u32, u32)]) -> Entry {
    let bytes = parts
        .iter()
        .flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes()))
        .collect();
    (tag, 5, u32::try_from(parts.len()).unwrap(), bytes)
}

fn byte(tag: u16, value: u8) -> Entry {
    (tag, 1, 1, vec![value])
}

/// A little-endian TIFF whose first IFD points to a GPS IFD holding `gps`.
fn tiff_with_gps(gps: &[Entry]) -> Vec<u8> {
    let ifd_len = |n: usize| 2 + 12 * n + 4;
    let gps_at = 8 + ifd_len(1);
    let mut out = b"II*\0".to_vec();
    out.extend_from_slice(&8u32.to_le_bytes());
    // IFD0: one entry, the GPS pointer.
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0x8825u16.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(gps_at).unwrap().to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    // The GPS IFD, with values over four bytes after it.
    out.extend_from_slice(&u16::try_from(gps.len()).unwrap().to_le_bytes());
    let mut spill = gps_at + ifd_len(gps.len());
    let mut tail = Vec::new();
    for (tag, kind, count, bytes) in gps {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if bytes.len() <= 4 {
            let mut inline = bytes.clone();
            inline.resize(4, 0);
            out.extend_from_slice(&inline);
        } else {
            out.extend_from_slice(&u32::try_from(spill).unwrap().to_le_bytes());
            spill += bytes.len();
            tail.extend_from_slice(bytes);
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&tail);
    out
}

fn place(lat_ref: &str, lon_ref: &str) -> Vec<Entry> {
    vec![
        ascii(1, lat_ref),
        rationals(2, &[(37, 1), (46, 1), (2964, 100)]),
        ascii(3, lon_ref),
        rationals(4, &[(122, 1), (25, 1), (984, 100)]),
    ]
}

fn read(gps: &[Entry]) -> Option<Location> {
    Location::of_file(&tiff_with_gps(gps))
}

fn micro(location: Option<Location>) -> Option<String> {
    location.map(|l| format!("{:?}", l.place))
}

#[test]
fn hemispheres_give_the_sign() {
    // name, latitude ref, longitude ref, words
    const CASES: &[(&str, &str, &str, &str)] = &[
        ("north west", "N", "W", "37.7749° N, 122.4194° W"),
        ("south east", "S", "E", "37.7749° S, 122.4194° E"),
        ("north east", "N", "E", "37.7749° N, 122.4194° E"),
        ("south west", "S", "W", "37.7749° S, 122.4194° W"),
        ("lower case", "s", "e", "37.7749° S, 122.4194° E"),
    ];
    for (name, lat, lon, want) in CASES {
        let location = read(&place(lat, lon)).unwrap_or_else(|| panic!("{name}: no place"));
        assert_eq!(location.facts().rows()[0].value.as_str(), *want, "{name}");
    }
    let south_east = micro(read(&place("S", "E"))).unwrap();
    assert!(south_east.contains("latitude: -37774900"), "{south_east}");
    assert!(south_east.contains("longitude: 122419400"), "{south_east}");
}

#[test]
fn a_missing_or_garbled_reference_gives_no_place() {
    let without =
        |tag: u16| -> Vec<Entry> { place("N", "E").into_iter().filter(|e| e.0 != tag).collect() };
    assert_eq!(read(&without(1)), None, "no latitude ref");
    assert_eq!(read(&without(3)), None, "no longitude ref");
    assert_eq!(read(&place("X", "E")), None, "latitude ref X");
    assert_eq!(read(&place("N", "")), None, "empty longitude ref");
    assert!(read(&place("N", "E")).is_some());
}

#[test]
fn a_receiver_without_a_fix_gives_no_place() {
    let mut void = place("N", "E");
    void.push(ascii(9, "V"));
    assert_eq!(read(&void), None, "status V");
    let mut fixed = place("N", "E");
    fixed.push(ascii(9, "A"));
    assert!(read(&fixed).is_some(), "status A");
    let null_island = vec![
        ascii(1, "N"),
        rationals(2, &[(0, 1), (0, 1), (0, 1)]),
        ascii(3, "E"),
        rationals(4, &[(0, 1), (0, 1), (0, 1)]),
    ];
    assert_eq!(read(&null_island), None, "0, 0");
}

#[test]
fn altitude_is_rounded_and_signed_by_its_reference() {
    let mut above = place("N", "E");
    above.push(rationals(6, &[(125, 10)]));
    assert_eq!(read(&above).unwrap().altitude, Some(13));
    let mut below = place("N", "E");
    below.push(rationals(6, &[(40, 1)]));
    below.push(byte(5, 1));
    assert_eq!(read(&below).unwrap().altitude, Some(-40));
}

fn rat(num: u32, denom: u32) -> Rational {
    Rational { num, denom }
}

#[test]
fn signed_turns_degrees_minutes_seconds_into_millionths() {
    let dms = |parts: [Rational; 3]| Value::Rational(parts.to_vec());
    let letter = |l: &str| Value::Ascii(vec![l.as_bytes().to_vec()]);
    let value = dms([rat(37, 1), rat(46, 1), rat(2964, 100)]);
    assert_eq!(signed(&value, &letter("N"), "N", "S"), Some(37_774_900));
    assert_eq!(signed(&value, &letter("S"), "N", "S"), Some(-37_774_900));
    assert_eq!(signed(&value, &letter("s"), "N", "S"), Some(-37_774_900));
    assert_eq!(signed(&value, &letter("Q"), "N", "S"), None);
    assert_eq!(signed(&value, &Value::Ascii(vec![]), "N", "S"), None);
    assert_eq!(signed(&value, &Value::Byte(vec![1]), "N", "S"), None);
    // Rounded to a millionth: 0.5 of a millionth rounds up.
    let rounding = dms([rat(0, 1), rat(0, 1), rat(18, 10_000)]);
    assert_eq!(signed(&rounding, &letter("N"), "N", "S"), Some(1));
    // A zero denominator, two parts, or a non-rational value is no place.
    assert_eq!(
        signed(
            &dms([rat(1, 0), rat(0, 1), rat(0, 1)]),
            &letter("N"),
            "N",
            "S"
        ),
        None
    );
    assert_eq!(
        signed(
            &Value::Rational(vec![rat(1, 1), rat(0, 1)]),
            &letter("N"),
            "N",
            "S"
        ),
        None
    );
    assert_eq!(signed(&Value::Byte(vec![1]), &letter("N"), "N", "S"), None);
    // The pole and the date line are as far as a place goes; beyond them `Coordinate` refuses.
    let max = dms([rat(180, 1), rat(0, 1), rat(0, 1)]);
    assert_eq!(signed(&max, &letter("E"), "E", "W"), Some(180_000_000));
}
