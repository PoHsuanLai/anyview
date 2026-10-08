//! The formats beyond photographs and stills: animation timing and runs from the container, the
//! float and layered formats, and the first frame a peek shows of an animation. Every file is
//! made here, so the pixels asked of it are the ones put in.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{
    ByteLen, FilePath, FileStamp, Input, MediaTime, ModTime, Peek, PixelLen, PixelSize, Source,
};
use anyview_image::{Decoded, FrameCount, Plays, RasterPeek, Rgba8, decode_bytes};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, DynamicImage, Frame, ImageFormat, Rgb, Rgb32FImage, RgbaImage};
use std::io::Cursor;
use std::num::NonZeroU32;
use support::{budget, sniffed};

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

fn at(picture: &Rgba8, x: u32, y: u32) -> [u8; 4] {
    let width = picture.size().width.0 as usize;
    let at = (y as usize * width + x as usize) * 4;
    picture.bytes()[at..at + 4].try_into().unwrap()
}

const COLOURS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];

fn gif(repeat: Option<Repeat>) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).unwrap();
        }
        for (colour, millis) in COLOURS.iter().zip([30, 80, 0]) {
            let picture = RgbaImage::from_pixel(8, 6, image::Rgba(*colour));
            let delay = Delay::from_numer_denom_ms(millis, 1);
            encoder
                .encode_frame(Frame::from_parts(picture, 0, 0, delay))
                .unwrap();
        }
    }
    out
}

fn apng(plays: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, 8, 6);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(3, plays).unwrap();
        encoder.set_frame_delay(25, 1000).unwrap();
        let mut writer = encoder.write_header().unwrap();
        for colour in COLOURS {
            let data: Vec<u8> = colour.iter().copied().cycle().take(8 * 6 * 4).collect();
            writer.write_image_data(&data).unwrap();
        }
    }
    out
}

fn animation(bytes: &[u8], name: &str) -> anyview_image::Animation {
    match decode_bytes(bytes, &sniffed(bytes, name)).unwrap() {
        Decoded::Animated(animation) => animation,
        other @ (Decoded::Still(_) | Decoded::HeldStill { .. }) => {
            panic!("{name} is not an animation: {other:?}")
        }
    }
}

#[test]
fn a_gif_plays_the_runs_its_loop_extension_asks_for() {
    // name, the extension written, runs
    let cases = [
        ("forever", Some(Repeat::Infinite), Plays::Forever),
        (
            "one repeat is two runs",
            Some(Repeat::Finite(1)),
            Plays::Times(NonZeroU32::new(2).unwrap()),
        ),
    ];
    for (name, repeat, plays) in cases {
        assert_eq!(animation(&gif(repeat), "a.gif").plays, plays, "{name}");
    }
}

#[test]
fn a_gif_frame_asking_for_no_time_is_shown_for_a_tenth_of_a_second() {
    let animation = animation(&gif(Some(Repeat::Infinite)), "a.gif");
    let delays: Vec<MediaTime> = animation.frames.iter().map(|f| f.delay).collect();
    // 30 ms and 80 ms are kept; 0 ms is too short to see.
    assert_eq!(delays, [30, 80, 100].map(MediaTime::from_millis));
    let colours: Vec<[u8; 4]> = animation
        .frames
        .iter()
        .map(|f| at(&f.pixels, 0, 0))
        .collect();
    assert_eq!(colours, COLOURS);
}

#[test]
fn an_apng_has_its_frames_delays_and_plays() {
    let animation = animation(&apng(3), "a.png");
    assert_eq!(animation.plays, Plays::Times(NonZeroU32::new(3).unwrap()));
    assert_eq!(animation.frames.count().get(), 3);
    assert!(
        animation
            .frames
            .iter()
            .all(|f| f.delay == MediaTime::from_millis(25))
    );
    let colours: Vec<[u8; 4]> = animation
        .frames
        .iter()
        .map(|f| at(&f.pixels, 3, 3))
        .collect();
    assert_eq!(colours, COLOURS);
    assert_eq!(self::animation(&apng(0), "a.png").plays, Plays::Forever);
}

#[test]
fn an_animated_webp_reports_the_runs_in_its_header() {
    let bytes = support::bytes("anim.webp");
    let animation = animation(&bytes, "anim.webp");
    // The fixture's ANIM chunk is read from the file, not assumed.
    assert_eq!(animation.frames.count().get(), 3);
    assert_eq!(animation.plays, Plays::Forever);
}

fn encoded(format: ImageFormat, values: [[f32; 3]; 4]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb32F(float_image(values))
        .write_to(&mut out, format)
        .unwrap();
    out.into_inner()
}

fn float_image(values: [[f32; 3]; 4]) -> Rgb32FImage {
    let mut image = Rgb32FImage::new(4, 2);
    for (index, pixel) in image.pixels_mut().enumerate() {
        *pixel = Rgb(values[index % 4]);
    }
    image
}

#[test]
fn an_exr_and_a_radiance_file_come_out_as_tone_mapped_srgb() {
    let values = [[0.0; 3], [1.0, 1.0, 1.0], [0.5, 0.5, 0.5], [0.0, 0.0, 1.0]];
    for (name, format) in [("a.exr", ImageFormat::OpenExr), ("a.hdr", ImageFormat::Hdr)] {
        let bytes = encoded(format, values);
        let Decoded::Still(picture) = decode_bytes(&bytes, &sniffed(&bytes, name)).unwrap() else {
            panic!("{name} is a still");
        };
        assert_eq!(picture.size(), size(4, 2), "{name}");
        assert_eq!(at(&picture, 0, 0), [0, 0, 0, 255], "{name}: black");
        assert_eq!(at(&picture, 1, 0), [255, 255, 255, 255], "{name}: white");
        // Radiance stores a shared exponent, so mid grey is within a step of sRGB 188.
        assert!(at(&picture, 2, 0)[0].abs_diff(188) <= 3, "{name}: mid grey");
        assert_eq!(at(&picture, 3, 0)[..2], [0, 0], "{name}: blue");
    }
}

#[test]
fn highlights_above_one_are_compressed_not_clipped() {
    let values = [[0.25; 3], [2.0; 3], [8.0; 3], [0.0; 3]];
    let bytes = encoded(ImageFormat::OpenExr, values);
    let Decoded::Still(picture) = decode_bytes(&bytes, &sniffed(&bytes, "a.exr")).unwrap() else {
        panic!("a still");
    };
    let (dim, bright, brightest) = (
        at(&picture, 0, 0)[0],
        at(&picture, 1, 0)[0],
        at(&picture, 2, 0)[0],
    );
    assert!(
        dim < bright && bright < brightest,
        "{dim} {bright} {brightest}"
    );
    assert_eq!(brightest, 255, "the largest value is white");
}

/// A Photoshop file: 8-bit RGB, no layers, uncompressed planes.
fn psd(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut out = b"8BPS".to_vec();
    out.extend(1u16.to_be_bytes());
    out.extend([0; 6]);
    out.extend(3u16.to_be_bytes());
    out.extend(height.to_be_bytes());
    out.extend(width.to_be_bytes());
    out.extend(8u16.to_be_bytes());
    out.extend(3u16.to_be_bytes());
    out.extend([0; 12]);
    out.extend(0u16.to_be_bytes());
    for channel in rgb {
        out.extend(std::iter::repeat_n(channel, (width * height) as usize));
    }
    out
}

#[test]
fn a_photoshop_file_decodes_to_its_flattened_picture() {
    let bytes = psd(5, 3, [200, 100, 50]);
    let Decoded::Still(picture) = decode_bytes(&bytes, &sniffed(&bytes, "a.psd")).unwrap() else {
        panic!("a still");
    };
    assert_eq!(picture.size(), size(5, 3));
    assert_eq!(at(&picture, 2, 1), [200, 100, 50, 255]);
}

#[test]
fn an_icon_family_decodes_to_its_largest_picture() {
    let mut family = icns::IconFamily::new();
    for (kind, side, grey) in [
        (icns::IconType::RGBA32_16x16, 16, 10),
        (icns::IconType::RGBA32_32x32, 32, 90),
    ] {
        let picture = RgbaImage::from_pixel(side, side, image::Rgba([grey; 4]));
        let mut png = Cursor::new(Vec::new());
        picture.write_to(&mut png, ImageFormat::Png).unwrap();
        family
            .elements
            .push(icns::IconElement::new(kind.ostype(), png.into_inner()));
    }
    let mut bytes = Vec::new();
    family.write(&mut bytes).unwrap();
    let Decoded::Still(picture) = decode_bytes(&bytes, &sniffed(&bytes, "a.icns")).unwrap() else {
        panic!("a still");
    };
    assert_eq!(picture.size(), size(32, 32));
    assert_eq!(at(&picture, 0, 0), [90; 4]);
}

fn on_disk(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> Source {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    Source::new(FilePath::new(path).unwrap(), stamp)
}

#[test]
fn a_peek_of_each_new_format_is_its_first_picture_and_an_animation_peeks_one_frame() {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        ("a.gif", gif(Some(Repeat::Infinite)), 3, size(8, 6)),
        ("a.png", apng(0), 3, size(8, 6)),
        ("a.psd", psd(5, 3, [1, 2, 3]), 1, size(5, 3)),
        (
            "a.hdr",
            encoded(ImageFormat::Hdr, [[0.5; 3]; 4]),
            1,
            size(4, 2),
        ),
    ];
    for (name, bytes, frames, whole) in files {
        let src = on_disk(&dir, name, &bytes);
        let peeked =
            RasterPeek::peek(&Input::from(&src), &sniffed(&bytes, name), &budget(10_000)).unwrap();
        assert_eq!(peeked.source_size, whole, "{name}");
        assert_eq!(peeked.picture.size(), whole, "{name}: one still frame");
        assert_eq!(peeked.frames, FrameCount(frames), "{name}");
    }
}
