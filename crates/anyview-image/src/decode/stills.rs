//! Formats the `image` crate decodes: one picture, or the frames of an animation.

use super::colour::ColourInfo;
use super::{Frame, MAX_ANIMATION_BYTES, MAX_DECODE_AREA};
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

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_AREA.0.saturating_mul(8));
    limits
}

fn decode_error(error: image::ImageError, size: Option<PixelSize>) -> ImageError {
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
fn declared_size(bytes: &[u8], format: ImageFormat) -> Result<PixelSize, ImageError> {
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

/// All the frames of an animation, refusing one that would take more memory than a viewer should
/// hold.
pub(crate) fn collect(frames: Frames<'_>) -> Result<Vec<Frame>, ImageError> {
    let mut out: Vec<Frame> = Vec::new();
    let mut spent: u64 = 0;
    for frame in frames {
        let frame = frame_of(frame.map_err(|e| decode_error(e, None))?);
        spent = spent.saturating_add(frame.pixels.bytes().len() as u64);
        if spent > MAX_ANIMATION_BYTES {
            return Err(ImageError::TooLarge {
                size: frame.pixels.size(),
            });
        }
        out.push(frame);
    }
    Ok(out)
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
}
