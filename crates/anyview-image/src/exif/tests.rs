use super::format;
use super::*;
use crate::orientation::Mirror;
use anyview_core::QuarterTurn;

const ROTATED: &[u8] = include_bytes!("../../tests/fixtures/rotated.jpg");
const PLAIN: &[u8] = include_bytes!("../../tests/fixtures/plain.jpg");

fn ratio(numerator: u32, denominator: u32) -> Ratio {
    Ratio {
        numerator,
        denominator,
    }
}

#[test]
fn the_rotated_fixture_reads_back_what_was_written_into_it() {
    let facts = ExifFacts::read(ROTATED);
    assert_eq!(facts.orientation.tag(), 6);
    assert_eq!(facts.orientation.turn, QuarterTurn::Quarter);
    assert_eq!(facts.orientation.mirror, Mirror::Unmirrored);
    assert_eq!(facts.camera().as_deref(), Some("TestCam One"));
    assert_eq!(facts.lens.as_deref(), Some("TestLens 35mm f/2"));
    assert_eq!(
        facts.exposure,
        Exposure {
            shutter: Some(ratio(1, 200)),
            aperture: Some(ratio(28, 10)),
            iso: Some(100),
            focal_length: Some(ratio(35, 1)),
        }
    );
    assert_eq!(
        facts.exposure_text().as_deref(),
        Some("1/200 s · f/2.8 · ISO 100 · 35 mm")
    );
    assert_eq!(facts.taken_text().as_deref(), Some("2024-05-01 12:30"));
}

#[test]
fn a_file_without_exif_has_no_facts_and_is_upright() {
    assert_eq!(ExifFacts::read(PLAIN), ExifFacts::none());
    assert_eq!(ExifFacts::read(b"not an image"), ExifFacts::none());
    assert_eq!(ExifFacts::none().camera(), None);
    assert_eq!(ExifFacts::none().exposure_text(), None);
    assert_eq!(ExifFacts::none().taken_text(), None);
}

#[test]
fn cameras_are_named_by_model_with_the_maker_when_it_adds_something() {
    // name, make, model, shown
    type Case = (
        &'static str,
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
    );
    const CASES: &[Case] = &[
        (
            "maker repeated",
            Some("Canon"),
            Some("Canon EOS R5"),
            Some("Canon EOS R5"),
        ),
        (
            "case differs",
            Some("CANON"),
            Some("Canon EOS R5"),
            Some("Canon EOS R5"),
        ),
        (
            "maker adds",
            Some("FUJIFILM"),
            Some("X-T4"),
            Some("FUJIFILM X-T4"),
        ),
        (
            "prefix is not a word",
            Some("Can"),
            Some("Canon EOS"),
            Some("Can Canon EOS"),
        ),
        ("model only", None, Some("iPhone 15"), Some("iPhone 15")),
        ("maker only", Some("Nikon"), None, Some("Nikon")),
        ("neither", None, None, None),
    ];
    for (name, make, model, want) in CASES {
        assert_eq!(format::camera(*make, *model).as_deref(), *want, "{name}");
    }
}

#[test]
fn exposures_read_as_a_photographer_writes_them() {
    // name, shutter, aperture, iso, focal length, shown
    type Case = (
        &'static str,
        Option<(u32, u32)>,
        Option<(u32, u32)>,
        Option<u32>,
        Option<(u32, u32)>,
        Option<&'static str>,
    );
    const CASES: &[Case] = &[
        (
            "all",
            Some((1, 250)),
            Some((18, 10)),
            Some(400),
            Some((50, 1)),
            Some("1/250 s · f/1.8 · ISO 400 · 50 mm"),
        ),
        ("long exposure", Some((5, 1)), None, None, None, Some("5 s")),
        (
            "fractional seconds",
            Some((13, 10)),
            None,
            None,
            None,
            Some("1.3 s"),
        ),
        (
            "reduced fraction",
            Some((10, 600)),
            None,
            None,
            None,
            Some("1/60 s"),
        ),
        (
            "whole aperture",
            None,
            Some((8, 1)),
            None,
            None,
            Some("f/8"),
        ),
        (
            "focal rounds",
            None,
            None,
            None,
            Some((245, 10)),
            Some("25 mm"),
        ),
        ("one second", Some((1, 1)), None, None, None, Some("1 s")),
        ("nothing", None, None, None, None, None),
    ];
    for (name, shutter, aperture, iso, focal, want) in CASES {
        let to_ratio = |pair: &Option<(u32, u32)>| pair.map(|(n, d)| ratio(n, d));
        let exposure = Exposure {
            shutter: to_ratio(shutter),
            aperture: to_ratio(aperture),
            iso: *iso,
            focal_length: to_ratio(focal),
        };
        assert_eq!(format::exposure(&exposure).as_deref(), *want, "{name}");
    }
}

#[test]
fn capture_times_are_shown_to_the_minute_or_left_alone() {
    const CASES: &[(&str, &str, &str)] = &[
        ("exif style", "2024:05:01 12:30:45", "2024-05-01 12:30"),
        ("no seconds", "2024:05:01 12:30", "2024-05-01 12:30"),
        ("already iso", "2024-05-01 12:30:45", "2024-05-01 12:30:45"),
        ("blank date", "    :  :     :  :  ", "    :  :     :  :  "),
        ("no time", "2024:05:01", "2024:05:01"),
    ];
    for (name, raw, want) in CASES {
        assert_eq!(format::taken(raw), *want, "{name}");
    }
}

/// A TIFF block with only a Make entry (`Ab`), written in the given byte order.
fn tiff_with_make(big: bool) -> Vec<u8> {
    let u16s = |v: u16| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let u32s = |v: u32| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let mut out = Vec::new();
    out.extend_from_slice(if big { b"MM\0*" } else { b"II*\0" });
    out.extend_from_slice(&u32s(8));
    out.extend_from_slice(&u16s(1));
    out.extend_from_slice(&u16s(0x010F));
    out.extend_from_slice(&u16s(2));
    out.extend_from_slice(&u32s(3));
    out.extend_from_slice(b"Ab\0\0");
    out.extend_from_slice(&u32s(0));
    out
}

fn read_raw(tiff: &[u8]) -> ExifFacts {
    // `read` takes a container; wrap the block as a bare TIFF file, which is one.
    ExifFacts::read(tiff)
}

#[test]
fn an_orientation_entry_is_added_when_the_block_has_none_and_the_rest_survives() {
    for big in [false, true] {
        let before = tiff_with_make(big);
        let turned = ExifOrientation::from_tag(8).unwrap();
        let after = with_orientation(&before, turned).unwrap();
        assert!(
            after.len() > before.len(),
            "a new IFD0 is appended, big endian {big}"
        );
        let facts = read_raw(&after);
        assert_eq!(facts.orientation, turned, "big endian {big}");
        assert_eq!(facts.make.as_deref(), Some("Ab"), "big endian {big}");
        assert_eq!(
            &after[8..before.len()],
            &before[8..],
            "old bytes kept, big endian {big}"
        );
    }
}

#[test]
fn an_existing_orientation_entry_changes_two_bytes_and_nothing_else() {
    // SOI, then the APP1 marker and its big-endian length, then "Exif\0\0" and the block.
    assert_eq!(&ROTATED[2..4], [0xFF, 0xE1]);
    let length = usize::from(u16::from_be_bytes([ROTATED[4], ROTATED[5]]));
    let block = &ROTATED[12..4 + length];
    let upright = ExifOrientation::UPRIGHT;
    let patched = with_orientation(block, upright).unwrap();
    let differing: Vec<usize> = (0..block.len())
        .filter(|&i| block[i] != patched[i])
        .collect();
    assert_eq!(patched.len(), block.len());
    assert_eq!(differing.len(), 1, "6 to 1 changes one byte of the short");
    assert_eq!(read_raw(&patched).orientation, upright);
}

#[test]
fn a_block_that_is_not_tiff_or_is_cut_off_is_refused() {
    const CASES: &[(&str, &[u8])] = &[
        ("empty", b""),
        ("wrong magic", b"XX*\0\x08\0\0\0"),
        ("no ifd", b"II*\0\x08\0\0\0"),
        ("cut entries", b"II*\0\x08\0\0\0\x05\0\x01\0"),
    ];
    for (name, block) in CASES {
        assert!(
            with_orientation(block, ExifOrientation::UPRIGHT).is_err(),
            "{name}"
        );
    }
}
