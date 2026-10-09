//! Encode then decode: lossless targets give the pixels back, lossy ones stay close, and metadata
//! carried from the original arrives upright.

#![cfg(feature = "encode")]

use crate::support;

use anyview_core::{MetadataCarry, Percent, Quality, RasterFormat, RasterTarget};
use anyview_image::{
    Decoded, ExifFacts, ImageError, Rgba8, decode_bytes, encode, encode_bmp, encode_with_metadata,
};
use exif::{Context, In, Rational, Tag, Value};
use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};
use support::{bytes, sniffed};

fn picture(name: &str) -> Rgba8 {
    let file = bytes(name);
    match decode_bytes(&file, &sniffed(&file, name)).unwrap() {
        Decoded::Still(picture) => picture,
        Decoded::Animated(_) | Decoded::HeldStill { .. } => panic!("{name} is a still"),
    }
}

fn back(file: &[u8], name: &str) -> Rgba8 {
    match decode_bytes(file, &sniffed(file, name)).unwrap() {
        Decoded::Still(picture) => picture,
        Decoded::Animated(_) | Decoded::HeldStill { .. } => panic!("{name} is a still"),
    }
}

fn quality(percent: u16) -> Quality {
    Quality::clamped(Percent(percent))
}

#[test]
fn lossless_targets_give_back_exactly_the_pixels_they_were_given() {
    let original = picture("quadrants.png");
    // name, file name to sniff, encoded
    let cases: Vec<(&str, &str, Vec<u8>)> = vec![
        (
            "png",
            "a.png",
            encode(&original, RasterTarget::Png).unwrap(),
        ),
        (
            "webp",
            "a.webp",
            encode(&original, RasterTarget::Webp).unwrap(),
        ),
        (
            "tiff",
            "a.tiff",
            encode(&original, RasterTarget::Tiff).unwrap(),
        ),
        ("bmp", "a.bmp", encode_bmp(&original).unwrap()),
    ];
    for (name, file, encoded) in &cases {
        assert_eq!(&back(encoded, file), &original, "{name}");
    }
}

#[test]
fn a_jpeg_keeps_the_size_stays_close_and_gets_smaller_at_lower_quality() {
    let original = picture("quadrants.png");
    let high = encode(&original, RasterTarget::Jpeg(quality(95))).unwrap();
    let low = encode(&original, RasterTarget::Jpeg(quality(10))).unwrap();
    assert!(low.len() < high.len(), "{} < {}", low.len(), high.len());
    let decoded = back(&high, "a.jpg");
    assert_eq!(decoded.size(), original.size());
    // The opaque green corner survives; the half-transparent red corner was flattened onto white.
    let width = original.size().width.0 as usize;
    let pixel = |p: &Rgba8, x: usize, y: usize| -> [u8; 4] {
        let i = (y * width + x) * 4;
        p.bytes()[i..i + 4].try_into().unwrap()
    };
    let green = pixel(&decoded, width - 1, 0);
    assert!(green[1] > 240 && green[0] < 20, "{green:?}");
    let flattened = pixel(&decoded, 0, 0);
    assert_eq!(flattened[3], 255);
    assert!(flattened[1] > 100, "red over white is pink: {flattened:?}");
}

#[test]
fn an_avif_is_a_valid_container_that_sniffs_as_avif() {
    let original = picture("quadrants.png");
    let encoded = encode(&original, RasterTarget::Avif(quality(60))).unwrap();
    assert_eq!(&encoded[4..12], b"ftypavif");
    let kind = sniffed(&encoded, "out.avif");
    assert_eq!(
        kind.detail(),
        &anyview_core::FormatDetail::Raster(RasterFormat::Avif)
    );
}

#[cfg(feature = "avif")]
#[test]
fn an_avif_round_trips_close_to_the_original() {
    let original = picture("plain.jpg");
    let encoded = encode(&original, RasterTarget::Avif(quality(90))).unwrap();
    assert_eq!(back(&encoded, "a.avif").size(), original.size());
}

#[test]
fn exif_carried_to_a_new_jpeg_arrives_upright_and_is_not_applied_twice() {
    let original_file = bytes("rotated.jpg");
    let upright = picture("rotated.jpg"); // 32 x 48, orientation already applied
    for (name, target, file) in [
        ("jpeg", RasterTarget::Jpeg(quality(90)), "o.jpg"),
        ("png", RasterTarget::Png, "o.png"),
        ("webp", RasterTarget::Webp, "o.webp"),
    ] {
        let kept =
            encode_with_metadata(&upright, target, &original_file, MetadataCarry::Keep).unwrap();
        let facts = ExifFacts::read(&kept);
        assert_eq!(facts.camera().as_deref(), Some("TestCam One"), "{name}");
        assert_eq!(facts.orientation.tag(), 1, "{name} is reset to upright");
        assert_eq!(
            back(&kept, file).size(),
            upright.size(),
            "{name} is not turned again"
        );
    }
}

#[test]
fn dropping_metadata_writes_none() {
    let original_file = bytes("rotated.jpg");
    let upright = picture("rotated.jpg");
    let dropped = encode_with_metadata(
        &upright,
        RasterTarget::Jpeg(quality(90)),
        &original_file,
        MetadataCarry::Drop,
    )
    .unwrap();
    assert_eq!(ExifFacts::read(&dropped), ExifFacts::none());
}

#[test]
fn an_icc_profile_travels_between_containers() {
    let profile = b"not a real profile, but bytes".to_vec();
    let png = bytes("quadrants.png");
    let mut original = DynImage::from_bytes(Bytes::from(png)).unwrap().unwrap();
    original.set_icc_profile(Some(Bytes::from(profile.clone())));
    let original = original.encoder().bytes().to_vec();
    let pixels = back(&original, "a.png");
    let jpeg = encode_with_metadata(
        &pixels,
        RasterTarget::Jpeg(quality(80)),
        &original,
        MetadataCarry::Keep,
    )
    .unwrap();
    let carried = DynImage::from_bytes(Bytes::from(jpeg))
        .unwrap()
        .unwrap()
        .icc_profile();
    assert_eq!(carried.as_deref(), Some(profile.as_slice()));
}

/// A JPEG with a camera, an orientation, a GPS position with a text value that is stored out of
/// line (so it would stay in the block if only the pointer were removed) and an ICC profile.
fn located_jpeg(profile: &[u8]) -> Vec<u8> {
    let rational = |num| Value::Rational(vec![Rational { num, denom: 1 }]);
    let fields = [
        field(Tag::Make, Value::Ascii(vec![b"TestCam".to_vec()])),
        field(Tag::Model, Value::Ascii(vec![b"One".to_vec()])),
        field(Tag::Orientation, Value::Short(vec![1])),
        field(Tag::GPSLatitudeRef, Value::Ascii(vec![b"N".to_vec()])),
        field(Tag::GPSLatitude, rational(37)),
        field(Tag::GPSMapDatum, Value::Ascii(vec![SECRET.to_vec()])),
    ];
    let mut writer = exif::experimental::Writer::new();
    for f in &fields {
        writer.push_field(f);
    }
    let mut block = std::io::Cursor::new(Vec::new());
    writer.write(&mut block, false).unwrap();
    let mut image = DynImage::from_bytes(Bytes::from(bytes("plain.jpg")))
        .unwrap()
        .unwrap();
    image.set_exif(Some(Bytes::from(block.into_inner())));
    image.set_icc_profile(Some(Bytes::copy_from_slice(profile)));
    image.encoder().bytes().to_vec()
}

const SECRET: &[u8] = b"SECRET-PLACE-NAME";

fn field(tag: Tag, value: Value) -> exif::Field {
    exif::Field {
        tag,
        ifd_num: In::PRIMARY,
        value,
    }
}

fn raw_fields(file: &[u8]) -> Vec<exif::Field> {
    let block = DynImage::from_bytes(Bytes::copy_from_slice(file))
        .unwrap()
        .unwrap()
        .exif()
        .expect("the file has an EXIF block");
    exif::Reader::new()
        .read_raw(block.to_vec())
        .unwrap()
        .fields()
        .cloned()
        .collect()
}

fn has_location(fields: &[exif::Field]) -> bool {
    fields
        .iter()
        .any(|f| f.tag.0 == Context::Gps || f.tag == Tag::GPSInfoIFDPointer)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn stripping_the_location_removes_the_gps_directory_and_keeps_the_camera() {
    let original = located_jpeg(b"profile");
    assert!(has_location(&raw_fields(&original)), "the fixture has GPS");
    let upright = back(&original, "o.jpg");
    for (name, target) in [
        ("jpeg", RasterTarget::Jpeg(quality(90))),
        ("png", RasterTarget::Png),
        ("webp", RasterTarget::Webp),
    ] {
        let out = encode_with_metadata(&upright, target, &original, MetadataCarry::StripLocation)
            .unwrap();
        let fields = raw_fields(&out);
        assert!(!has_location(&fields), "{name} has no GPS tag");
        assert!(!contains(&out, SECRET), "{name} holds no GPS value bytes");
        assert_eq!(
            ExifFacts::read(&out).camera().as_deref(),
            Some("TestCam One"),
            "{name} keeps the camera"
        );
    }
}

#[test]
fn keeping_everything_keeps_the_location() {
    let original = located_jpeg(b"profile");
    let out = encode_with_metadata(
        &back(&original, "o.jpg"),
        RasterTarget::Jpeg(quality(90)),
        &original,
        MetadataCarry::Keep,
    )
    .unwrap();
    assert!(has_location(&raw_fields(&out)));
}

#[test]
fn the_default_carry_strips_the_location() {
    assert_eq!(MetadataCarry::default(), MetadataCarry::StripLocation);
}

#[test]
fn the_colour_profile_travels_whatever_is_chosen_about_the_rest() {
    let profile = b"a wide-gamut profile, as bytes".to_vec();
    let original = located_jpeg(&profile);
    let pixels = back(&original, "o.jpg");
    for keep in [
        MetadataCarry::Keep,
        MetadataCarry::StripLocation,
        MetadataCarry::Drop,
    ] {
        for (target, file) in [
            (RasterTarget::Jpeg(quality(80)), "o.jpg"),
            (RasterTarget::Png, "o.png"),
            (RasterTarget::Webp, "o.webp"),
        ] {
            let out = encode_with_metadata(&pixels, target, &original, keep).unwrap();
            let icc = DynImage::from_bytes(Bytes::from(out))
                .unwrap()
                .unwrap()
                .icc_profile();
            assert_eq!(icc.as_deref(), Some(profile.as_slice()), "{keep:?} {file}");
        }
    }
}

#[test]
fn dropping_still_leaves_no_exif_but_keeps_the_profile() {
    let original = located_jpeg(b"profile");
    let out = encode_with_metadata(
        &back(&original, "o.jpg"),
        RasterTarget::Png,
        &original,
        MetadataCarry::Drop,
    )
    .unwrap();
    assert_eq!(ExifFacts::read(&out), ExifFacts::none());
    assert!(!contains(&out, SECRET));
}

#[test]
fn an_avif_carries_the_profile_and_no_location_unless_asked() {
    let profile = b"a wide-gamut profile, as bytes".to_vec();
    let original = located_jpeg(&profile);
    let pixels = back(&original, "o.jpg");
    let target = RasterTarget::Avif(quality(40));
    for keep in [MetadataCarry::StripLocation, MetadataCarry::Drop] {
        let out = encode_with_metadata(&pixels, target, &original, keep).unwrap();
        assert!(contains(&out, &profile), "{keep:?} carries the profile");
        assert!(!contains(&out, SECRET), "{keep:?} has no location");
        if cfg!(feature = "avif") {
            assert_eq!(back(&out, "o.avif").size(), pixels.size(), "still decodes");
        }
    }
    let kept = encode_with_metadata(&pixels, target, &original, MetadataCarry::Keep).unwrap();
    assert!(contains(&kept, SECRET), "Keep means all of it");
    let stripped =
        encode_with_metadata(&pixels, target, &original, MetadataCarry::StripLocation).unwrap();
    assert!(contains(&stripped, b"TestCam"), "the camera stays");
}

#[test]
fn an_original_without_metadata_leaves_the_encoding_untouched() {
    let original_file = bytes("plain.jpg");
    let pixels = picture("plain.jpg");
    let plain = encode(&pixels, RasterTarget::Png).unwrap();
    let kept = encode_with_metadata(
        &pixels,
        RasterTarget::Png,
        &original_file,
        MetadataCarry::Keep,
    )
    .unwrap();
    assert_eq!(kept, plain);
}

#[test]
fn a_buffer_of_the_wrong_length_is_refused_before_any_encoder_sees_it() {
    let size = anyview_core::PixelSize {
        width: anyview_core::PixelLen(2),
        height: anyview_core::PixelLen(2),
    };
    assert!(matches!(
        Rgba8::new(size, vec![0; 15]),
        Err(ImageError::PixelsMismatch { .. })
    ));
}
