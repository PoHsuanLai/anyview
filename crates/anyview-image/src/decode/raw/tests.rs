//! The preview of a raw file, over raw files built here: a TIFF with a JPEG preview, a thumbnail
//! and sensor data that is not a preview.

use super::*;
use image::{ImageEncoder, RgbImage, codecs::jpeg::JpegEncoder};

/// A JPEG of `width` x `height` whose left half is red and right half blue.
fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            image::Rgb([255, 0, 0])
        } else {
            image::Rgb([0, 0, 255])
        }
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 95)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    out
}

/// Sensor data stored as lossless JPEG (frame type SOF3), which is no preview.
fn lossless() -> Vec<u8> {
    let mut data = vec![0xFF, 0xD8, 0xFF, 0xC4, 0x00, 0x04, 0x00, 0x00];
    // SOF3: length 11, 12 bits, 4000 x 6000, one component.
    data.extend([
        0xFF, 0xC3, 0x00, 0x0B, 12, 0x17, 0x70, 0x0F, 0xA0, 1, 1, 0x11, 0,
    ]);
    data.extend([
        0xFF, 0xDA, 0x00, 0x08, 1, 1, 0, 1, 0, 0, 0x12, 0x34, 0xFF, 0xD9,
    ]);
    data
}

/// A little-endian TIFF whose first IFD holds an Orientation of `orientation` (when given),
/// followed by `blobs` in the order given.
fn tiff(orientation: Option<u16>, blobs: &[&[u8]]) -> Vec<u8> {
    let mut file = b"II*\0\x08\0\0\0".to_vec();
    match orientation {
        Some(tag) => {
            file.extend(1u16.to_le_bytes());
            file.extend(0x0112u16.to_le_bytes());
            file.extend(3u16.to_le_bytes());
            file.extend(1u32.to_le_bytes());
            file.extend(tag.to_le_bytes());
            file.extend([0, 0]);
        }
        None => file.extend(0u16.to_le_bytes()),
    }
    file.extend(0u32.to_le_bytes());
    for blob in blobs {
        file.extend(*blob);
        file.extend([0; 5]);
    }
    file
}

fn pixel(picture: &Rgba8, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * picture.size().width.0 + x) * 4) as usize;
    let bytes = picture.bytes();
    [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[2] < 60
}

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 60
}

#[test]
fn the_largest_decodable_jpeg_is_the_preview() {
    let thumb = jpeg(16, 8);
    let preview = jpeg(64, 32);
    let file = tiff(None, &[&thumb, &lossless(), &preview]);
    let found = largest_preview(&file).unwrap();
    assert_eq!((found.size.width.0, found.size.height.0), (64, 32));
    assert_eq!(&file[found.at], preview.as_slice());
}

#[test]
fn a_file_with_only_sensor_data_or_truncated_jpegs_has_no_preview() {
    let cut = jpeg(64, 32);
    let cut = &cut[..cut.len() / 2];
    for (name, file) in [
        ("lossless only", tiff(None, &[&lossless()])),
        ("truncated", tiff(None, &[cut])),
        ("no jpeg at all", tiff(Some(1), &[])),
        ("empty", Vec::new()),
    ] {
        assert_eq!(largest_preview(&file), None, "{name}");
    }
    assert_eq!(
        decode(b"II*\0\x08\0\0\0\0\0\0\0\0"),
        Err(ImageError::NoPreview)
    );
}

#[test]
fn a_preview_without_its_own_orientation_takes_the_one_in_the_raw_files_first_ifd() {
    // name, tag, size after turning, what is at the top-left
    type Case = (&'static str, Option<u16>, (u32, u32), fn([u8; 4]) -> bool);
    let cases: &[Case] = &[
        ("none", None, (64, 32), is_red),
        ("upright", Some(1), (64, 32), is_red),
        ("turned a quarter", Some(6), (32, 64), is_red),
        ("turned the other way", Some(8), (32, 64), is_blue),
        ("half", Some(3), (64, 32), is_blue),
    ];
    for (name, tag, size, top_left) in cases {
        let file = tiff(*tag, &[&jpeg(64, 32)]);
        let picture = decode(&file).unwrap();
        assert_eq!(
            (picture.size().width.0, picture.size().height.0),
            *size,
            "{name}"
        );
        assert!(
            top_left(pixel(&picture, 0, 0)),
            "{name}: {:?}",
            pixel(&picture, 0, 0)
        );
    }
}

#[test]
fn the_orientation_reader_takes_either_byte_order_and_makers_header_versions() {
    let mut big = b"MM\0*\0\0\0\x08".to_vec();
    big.extend([0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0]);
    assert_eq!(file_orientation(&big), ExifOrientation::from_tag(6));
    // Olympus and Panasonic raw files put their own number where TIFF has 42.
    let mut orf = tiff(Some(8), &[]);
    orf[2..4].copy_from_slice(b"RO");
    assert_eq!(file_orientation(&orf), ExifOrientation::from_tag(8));
    assert_eq!(file_orientation(&tiff(None, &[])), None);
    assert_eq!(file_orientation(b"not a tiff"), None);
    assert_eq!(file_orientation(b"II"), None);
}

#[test]
fn a_preview_in_an_iso_media_file_is_found_too() {
    // Canon's CR3 keeps a JPEG in a box of its own.
    let preview = jpeg(48, 32);
    let mut file = b"\0\0\0\x18ftypcrx \0\0\0\x01crx isom".to_vec();
    file.extend(((preview.len() + 8) as u32).to_be_bytes());
    file.extend(b"PRVW");
    file.extend(&preview);
    let picture = decode(&file).unwrap();
    assert_eq!((picture.size().width.0, picture.size().height.0), (48, 32));
}
