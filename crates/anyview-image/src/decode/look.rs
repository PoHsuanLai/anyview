//! A first look at an image file for a peek: its first picture, how many there are, and how it
//! stores its colour. Decodes as little as the format allows.

use super::ceiling::Ceiling;
use super::codec::{Codec, codec_for};
use super::colour::ColourInfo;
use super::{FrameCount, frame_count, highrange, jxl, layered, raw, stills, svg};
use crate::error::ImageError;
use crate::exif::ExifFacts;
use crate::pixels::Rgba8;
use anyview_core::{PixelArea, PixelSize, Sniffed};

/// What a peek learns about a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Looked {
    /// The first picture, upright. A raster is at its full size; an SVG is already drawn small
    /// enough for the budget it was asked about.
    pub picture: Rgba8,
    /// The size of the whole picture, upright, which the facts report.
    pub source_size: PixelSize,
    /// How many pictures the file holds.
    pub frames: FrameCount,
    /// How the file stores its colour, when the format says.
    pub colour: Option<ColourInfo>,
    /// The EXIF facts, or none.
    pub exif: ExifFacts,
}

/// The first picture of `bytes` and what the file says about itself. `area` is how many pixels the
/// caller can use: a vector image is drawn at that size and no larger.
pub(crate) fn look(bytes: &[u8], sniffed: &Sniffed, area: PixelArea) -> Result<Looked, ImageError> {
    let ceiling = Ceiling::PEEK;
    match codec_for(sniffed)? {
        Codec::Image(format) => match stills::animation(bytes, format, ceiling)? {
            Some(frames) => look_at_frames(frames, bytes, format),
            None => {
                let (picture, colour) = stills::still(bytes, format, ceiling)?;
                Ok(raster(
                    picture,
                    FrameCount(1),
                    Some(colour),
                    ExifFacts::read(bytes),
                ))
            }
        },
        Codec::HighRange(format) => {
            let (picture, colour) = highrange::decode(bytes, format, ceiling)?;
            Ok(raster(
                picture,
                FrameCount(1),
                Some(colour),
                ExifFacts::none(),
            ))
        }
        Codec::Psd => {
            let (picture, colour) = layered::psd(bytes, ceiling)?;
            Ok(raster(
                picture,
                FrameCount(1),
                Some(colour),
                ExifFacts::none(),
            ))
        }
        Codec::Icns => {
            let (picture, colour) = layered::icns(bytes)?;
            Ok(raster(
                picture,
                FrameCount(1),
                Some(colour),
                ExifFacts::none(),
            ))
        }
        Codec::Jxl => {
            let (picture, colour) = jxl::decode(bytes, ceiling)?;
            Ok(raster(
                picture,
                FrameCount(1),
                Some(colour),
                ExifFacts::none(),
            ))
        }
        Codec::RawPreview => {
            let picture = raw::decode(bytes, ceiling)?;
            Ok(raster(picture, FrameCount(1), None, ExifFacts::read(bytes)))
        }
        Codec::Svg => {
            let document = svg::Svg::parse(bytes)?;
            let intrinsic = document.intrinsic();
            let picture = document.render(crate::scale::fit_area(intrinsic, area))?;
            Ok(Looked {
                picture,
                source_size: intrinsic,
                frames: FrameCount(1),
                colour: None,
                exif: ExifFacts::none(),
            })
        }
    }
}

fn raster(
    picture: Rgba8,
    frames: FrameCount,
    colour: Option<ColourInfo>,
    exif: ExifFacts,
) -> Looked {
    Looked {
        source_size: picture.size(),
        picture,
        frames,
        colour,
        exif,
    }
}

/// The first frame in full and the number of frames the container states. Only the first frame is
/// decoded: counting by decoding the rest is minutes of work for a few kilobytes of file.
fn look_at_frames(
    frames: image::Frames<'_>,
    bytes: &[u8],
    format: image::ImageFormat,
) -> Result<Looked, ImageError> {
    let first = stills::guarded(|| {
        frames
            .into_iter()
            .next()
            .ok_or(ImageError::Decode {
                reason: "the animation has no frames".to_owned(),
            })?
            .map(|frame| stills::frame_of(frame).pixels)
            .map_err(|e| stills::decode_error(e, None))
    })?;
    let count = frame_count::of(bytes, format).map_or(1, |count| count.max(1));
    let colour = ColourInfo::of_channels(4, 8);
    Ok(raster(
        first,
        FrameCount(count),
        Some(colour),
        ExifFacts::read(bytes),
    ))
}
