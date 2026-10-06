//! Formats the `image` crate decodes: one picture, or the frames of an animation.

use super::Frame;
use super::ceiling::Ceiling;
use super::colour::ColourInfo;
use crate::error::ImageError;
use crate::exif::ExifFacts;
use crate::pixels::Rgba8;
use anyview_core::{MediaTime, PixelLen, PixelSize};
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, Frames, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Frame delays at or under this many milliseconds are shown as [`FALLBACK_DELAY_MS`]: files that
/// ask for no delay are meant to play at a readable speed, as browsers show them.
const SHORTEST_DELAY_MS: u64 = 10;
const FALLBACK_DELAY_MS: u64 = 100;

/// What a still costs per pixel beyond the codec's own buffer at its peak: the RGBA8 copy and the
/// copy that turns it upright.
pub(crate) const STILL_COPIES: u64 = 8;

/// What each frame of an animation costs per pixel at its peak: the decoder's canvas, the frame it
/// composites and the copy handed out, all RGBA8.
const ANIMATION_COPIES: u64 = 12;

pub(crate) fn limits(ceiling: Ceiling) -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(ceiling.bytes());
    limits
}

/// What a file's header says about its picture.
pub(crate) struct Header {
    /// The size it declares.
    pub size: PixelSize,
    /// The bytes per pixel of the codec's own buffer, in the colour depth the file stores.
    pub source_bytes: u64,
}

impl Header {
    /// The size and the bytes per pixel a decode costs at its peak, `copies` being what it holds
    /// besides the codec's own buffer.
    pub(crate) fn cost(&self, copies: u64) -> (PixelSize, u64) {
        (self.size, self.source_bytes + copies)
    }
}

/// A decoder's guard against a panic in a codec: the `image` crate's formats index into bytes
/// they have not checked, so a file that breaks one is reported as undecodable instead of taking
/// the worker down.
pub(crate) fn guarded<T>(work: impl FnOnce() -> Result<T, ImageError>) -> Result<T, ImageError> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Err(ImageError::Decode {
            reason: "the image is damaged".to_owned(),
        })
    })
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

/// The size an image file declares and its colour depth, read from its header only.
pub(crate) fn header(bytes: &[u8], format: ImageFormat) -> Result<Header, ImageError> {
    let decoder = ImageReader::with_format(Cursor::new(bytes), format)
        .into_decoder()
        .map_err(|e| decode_error(e, None))?;
    let (width, height) = decoder.dimensions();
    Ok(Header {
        size: PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        },
        source_bytes: u64::from(decoder.color_type().bytes_per_pixel()),
    })
}

/// The picture of a still image file, upright, with the colour it was stored in.
pub(crate) fn still(
    bytes: &[u8],
    format: ImageFormat,
    ceiling: Ceiling,
) -> Result<(Rgba8, ColourInfo), ImageError> {
    let (size, per_pixel) = header(bytes, format)?.cost(STILL_COPIES);
    ceiling.admit(size, per_pixel)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits(ceiling));
    let decoded = guarded(|| reader.decode().map_err(|e| decode_error(e, Some(size))))?;
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
    ceiling: Ceiling,
) -> Result<Option<Frames<'_>>, ImageError> {
    guarded(|| open_animation(bytes, format, ceiling))
}

fn open_animation(
    bytes: &[u8],
    format: ImageFormat,
    ceiling: Ceiling,
) -> Result<Option<Frames<'_>>, ImageError> {
    let wrap = |e| decode_error(e, None);
    let admit = |decoder: &dyn ImageDecoder| {
        let (width, height) = decoder.dimensions();
        let size = PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        };
        ceiling.admit(size, ANIMATION_COPIES)
    };
    if format == ImageFormat::Gif {
        let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(wrap)?;
        admit(&decoder)?;
        Ok(Some(decoder.into_frames()))
    } else if format == ImageFormat::Png {
        let decoder = PngDecoder::new(Cursor::new(bytes)).map_err(wrap)?;
        if decoder.is_apng().map_err(wrap)? {
            admit(&decoder)?;
            Ok(Some(decoder.apng().map_err(wrap)?.into_frames()))
        } else {
            Ok(None)
        }
    } else if format == ImageFormat::WebP {
        let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(wrap)?;
        if decoder.has_animation() {
            admit(&decoder)?;
            Ok(Some(decoder.into_frames()))
        } else {
            Ok(None)
        }
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
/// it decoding stops: the first frame alone is kept, and `total`, what the container says the
/// count is, tells how many there are. A frame decoded is a canvas of memory and time, so a file
/// of thousands is never decoded to the end to be counted.
pub(crate) fn collect(
    frames: Frames<'_>,
    cap: u64,
    total: Option<u32>,
) -> Result<Collected, ImageError> {
    guarded(|| {
        let mut kept: Vec<Frame> = Vec::new();
        let mut spent: u64 = 0;
        let mut seen: u32 = 0;
        for frame in frames {
            let frame = frame_of(frame.map_err(|e| decode_error(e, None))?);
            seen = seen.saturating_add(1);
            spent = spent.saturating_add(frame.pixels.bytes().len() as u64);
            if spent > cap {
                kept.truncate(1);
                if kept.is_empty() {
                    kept.push(frame);
                }
                let first = kept.remove(0);
                let count = total.unwrap_or(seen).max(seen);
                return Ok(Collected::TooMany { first, count });
            }
            kept.push(frame);
        }
        Ok(Collected::All(kept))
    })
}

#[cfg(test)]
mod tests {
    use super::super::frame_count;
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
            let frames = animation(&bytes, ImageFormat::Gif, Ceiling::VIEW)
                .unwrap()
                .unwrap();
            match collect(frames, *cap, frame_count::of(&bytes, ImageFormat::Gif)).unwrap() {
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
