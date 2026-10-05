//! Formats the `image` crate decodes: one picture, or the frames of an animation.

use super::colour::ColourInfo;
use super::{Frame, MAX_DECODE_AREA};
use crate::error::ImageError;
use crate::exif::ExifFacts;
use crate::pixels::Rgba8;
use anyview_core::{MediaTime, PixelLen, PixelSize};
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, Frames, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

/// Frame delays at or under this many milliseconds are shown as [`FALLBACK_DELAY_MS`]: files that
/// ask for no delay are meant to play at a readable speed, as browsers show them.
const SHORTEST_DELAY_MS: u64 = 10;
const FALLBACK_DELAY_MS: u64 = 100;

pub(crate) fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_AREA.0.saturating_mul(8));
    limits
}

pub(crate) fn decode_error(error: image::ImageError, size: Option<PixelSize>) -> ImageError {
    match (error, size) {
        (image::ImageError::Limits(_), Some(size)) => ImageError::TooLarge { size },
        (error, _) => ImageError::Decode {
            reason: error.to_string(),
        },
    }
}

/// A frame delay of `numerator / denominator` milliseconds as media time, with the browser rule
/// for delays too short to see.
pub(crate) fn frame_delay(numerator: u32, denominator: u32) -> MediaTime {
    let denominator = u64::from(denominator).max(1);
    let millis = (u64::from(numerator) + denominator / 2) / denominator;
    match millis {
        0..=SHORTEST_DELAY_MS => MediaTime::from_millis(FALLBACK_DELAY_MS),
        _ => MediaTime::from_millis(millis),
    }
}

/// The size an image file declares, read from its header only.
pub(crate) fn declared_size(bytes: &[u8], format: ImageFormat) -> Result<PixelSize, ImageError> {
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| decode_error(e, None))?;
    Ok(PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    })
}

/// The picture of a still image file, upright, with the colour it was stored in.
pub(crate) fn still(bytes: &[u8], format: ImageFormat) -> Result<(Rgba8, ColourInfo), ImageError> {
    let size = declared_size(bytes, format)?;
    if size.area() > MAX_DECODE_AREA {
        return Err(ImageError::TooLarge { size });
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits());
    let decoded = reader.decode().map_err(|e| decode_error(e, Some(size)))?;
    let colour = ColourInfo::of_color_type(decoded.color());
    let picture = Rgba8::from_image(decoded.into_rgba8());
    let orientation = ExifFacts::read(bytes).orientation;
    Ok((orientation.applied(&picture), colour))
}

/// The frames of an animated file, or `None` when the file holds one picture. GIF always yields
/// frames (a one-frame GIF is a still to the caller); PNG only when it is an APNG; WebP only when
/// its header says animated.
pub(crate) fn animation(
    bytes: &[u8],
    format: ImageFormat,
) -> Result<Option<Frames<'_>>, ImageError> {
    let wrap = |e| decode_error(e, None);
    if format == ImageFormat::Gif {
        let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(wrap)?;
        Ok(Some(decoder.into_frames()))
    } else if format == ImageFormat::Png {
        let decoder = PngDecoder::new(Cursor::new(bytes)).map_err(wrap)?;
        if decoder.is_apng().map_err(wrap)? {
            Ok(Some(decoder.apng().map_err(wrap)?.into_frames()))
        } else {
            Ok(None)
        }
    } else if format == ImageFormat::WebP {
        let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(wrap)?;
        Ok(decoder.has_animation().then(|| decoder.into_frames()))
    } else {
        Ok(None)
    }
}

/// One decoded frame of an animation.
pub(crate) fn frame_of(frame: image::Frame) -> Frame {
    let (numerator, denominator) = frame.delay().numer_denom_ms();
    Frame {
        delay: frame_delay(numerator, denominator),
        pixels: Rgba8::from_image(frame.into_buffer()),
    }
}

/// What the frames of an animation came to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Collected {
    /// Every frame, in order.
    All(Vec<Frame>),
    /// Together the frames would have taken more than the cap: the first alone, and how many
    /// there are.
    TooMany { first: Frame, count: u32 },
}

/// The frames of an animation, keeping them while they take at most `cap` bytes together. Past
/// it only the first is kept, and the rest are decoded and dropped one at a time to count them.
pub(crate) fn collect(frames: Frames<'_>, cap: u64) -> Result<Collected, ImageError> {
    let mut kept: Vec<Frame> = Vec::new();
    let mut spent: u64 = 0;
    let mut count: u32 = 0;
    for frame in frames {
        let frame = frame_of(frame.map_err(|e| decode_error(e, None))?);
        count = count.saturating_add(1);
        spent = spent.saturating_add(frame.pixels.bytes().len() as u64);
        if spent > cap {
            kept.truncate(1);
            if kept.is_empty() {
                kept.push(frame);
            }
        } else {
            kept.push(frame);
        }
    }
    if spent <= cap {
        return Ok(Collected::All(kept));
    }
    let first = kept.into_iter().next().ok_or_else(|| ImageError::Decode {
        reason: "the animation has no frames".to_owned(),
    })?;
    Ok(Collected::TooMany { first, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_delays_follow_the_browser_rule() {
        // name, numerator ms, denominator, result in ms
        const CASES: &[(&str, u32, u32, u64)] = &[
            ("a normal delay", 40, 1, 40),
            ("rounds to the nearest ms", 125, 10, 13),
            ("exactly ten is too short", 10, 1, 100),
            ("zero is too short", 0, 1, 100),
            ("eleven is shown as asked", 11, 1, 11),
            ("a zero denominator is treated as one", 50, 0, 50),
        ];
        for (name, numerator, denominator, millis) in CASES {
            assert_eq!(
                frame_delay(*numerator, *denominator),
                MediaTime::from_millis(*millis),
                "{name}"
            );
        }
    }

    fn three_frame_gif() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut out);
            for grey in [10, 20, 30] {
                let picture =
                    image::RgbaImage::from_pixel(8, 6, image::Rgba([grey, grey, grey, 255]));
                encoder.encode_frame(image::Frame::new(picture)).unwrap();
            }
        }
        out
    }

    #[test]
    fn frames_over_the_cap_fall_back_to_the_first_and_a_count() {
        // Each frame is 8 x 6 x 4 = 192 bytes.
        // name, cap, frames kept, count reported
        const CASES: &[(&str, u64, usize, Option<u32>)] = &[
            ("room for all three", 576, 3, None),
            ("one byte short of all three", 575, 1, Some(3)),
            ("room for the first only", 192, 1, Some(3)),
            ("not even the first fits", 100, 1, Some(3)),
        ];
        let bytes = three_frame_gif();
        for (name, cap, kept, count) in CASES {
            let frames = animation(&bytes, ImageFormat::Gif).unwrap().unwrap();
            match collect(frames, *cap).unwrap() {
                Collected::All(all) => {
                    assert_eq!((all.len(), None), (*kept, *count), "{name}");
                }
                Collected::TooMany { first, count: seen } => {
                    assert_eq!(first.pixels.bytes()[0], 10, "{name}: the first frame");
                    assert_eq!((1, Some(seen)), (*kept, *count), "{name}");
                }
            }
        }
    }
}
