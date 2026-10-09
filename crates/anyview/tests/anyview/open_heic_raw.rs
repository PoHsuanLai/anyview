//! HEIC and camera raw files opened end to end under the harness with the binary's own wiring and no
//! plugin installed, as a person has them before `--with-plugin heif` and `--with-plugin raw`: a
//! HEIC is its facts and the package that would show it, and a raw file shows the JPEG preview inside
//! it, never a blank window.

use crate::support;

use ds_harness::{Driver, Query};
use image::{ImageEncoder, codecs::jpeg::JpegEncoder};
use support::{open, until};

/// A TIFF-based "raw" file: a first IFD with an Orientation of 1 and a 64 x 32 JPEG preview whose
/// left half is red and right half blue, then bytes standing for the sensor data.
fn raw_file() -> Vec<u8> {
    let image = image::RgbImage::from_fn(64, 32, |x, _| {
        if x < 32 {
            image::Rgb([255, 0, 0])
        } else {
            image::Rgb([0, 0, 255])
        }
    });
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 95)
        .write_image(image.as_raw(), 64, 32, image::ExtendedColorType::Rgb8)
        .unwrap();
    let mut file = b"II*\0\x08\0\0\0".to_vec();
    file.extend([1, 0, 0x12, 0x01, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
    file.extend(&jpeg);
    file.extend([7u8; 64]);
    file
}

#[test]
fn a_heic_with_no_plugin_is_its_facts_and_the_package_that_would_show_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("photo.heic");
    std::fs::write(
        &file,
        b"\0\0\0\x18ftypheic\0\0\0\0mif1heic and some more bytes",
    )
    .unwrap();
    let mut rig = open(&file, dir.path());
    until(&mut rig.harness, "the facts card", |harness| {
        harness.count(".viewer-peek") > 0
    });
    let text = rig.harness.text_of(".viewer").unwrap_or_default();
    assert!(
        text.contains("Needs") && text.contains("anyview-heif (to show it)"),
        "the row that names the package: {text}"
    );
    assert!(text.contains("image/heic"), "what the file is: {text}");
    assert_eq!(
        rig.harness.count(".viewer-raster"),
        0,
        "no picture is drawn"
    );
    assert!(
        rig.workforce.notices().is_empty(),
        "every job ran to its end and posted its result"
    );
}

#[test]
fn a_raw_file_with_no_plugin_shows_its_embedded_preview() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("IMG_0001.CR2");
    std::fs::write(&file, raw_file()).unwrap();
    let mut rig = open(&file, dir.path());
    until(&mut rig.harness, "the picture's stage", |harness| {
        harness.count(".viewer-raster") > 0
    });
    let mut left_is_red = false;
    until(&mut rig.harness, "the preview's pixels", |harness| {
        let drawn = harness.render().unwrap();
        let (x, y) = (drawn.width() / 2, drawn.height() / 2);
        let left = drawn.get_pixel(x - 20, y);
        let right = drawn.get_pixel(x + 20, y);
        left_is_red = left.0[0] > 200 && left.0[2] < 80 && right.0[2] > 200 && right.0[0] < 80;
        left_is_red
    });
    assert!(left_is_red);
    assert!(
        rig.workforce.notices().is_empty(),
        "every job ran to its end and posted its result"
    );
}
