//! Decoding: a file becomes straight RGBA8 pixels, upright, as one picture or as the frames of an
//! animation.
//!
//! Everything here is blocking and runs on the caller's worker. Nothing reads a clock or spawns.

mod ceiling;
mod codec;
mod colour;
mod frame_count;
mod highrange;
mod jxl;
mod layered;
mod look;
mod natural;
mod plays;
mod raw;
mod stills;
mod svg;
mod svg_limits;

pub use colour::{ColourInfo, ColourModel};
pub(crate) use look::{Looked, look};
pub use natural::natural_size;
pub use plays::Plays;
pub(crate) use plays::plays_of;
pub(crate) use svg::Svg;

use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::{MediaTime, NonEmpty, PixelArea, PixelSize, QuarterTurn, Sniffed, Source};
use ceiling::Ceiling;
use codec::{Codec, codec_for};
use stills::Collected;

/// The most pixels an SVG is drawn at: 16384 by 16384, a gibibyte of RGBA. Raster files are held
/// to [`Ceiling`] instead, which counts what their decode costs at its peak.
pub(crate) const MAX_DECODE_AREA: PixelArea = PixelArea(268_435_456);

/// The most memory the frames of one animation may take together: 256 MiB of RGBA8, which the
/// viewer holds twice, decoded and in one texture per frame. Past it the file opens as its first
/// frame alone ([`Decoded::HeldStill`]) rather than refusing or growing without bound.
pub(crate) const MAX_ANIMATION_BYTES: u64 = 256 * 1024 * 1024;

/// An SVG shown in full is drawn with its long edge at least this many pixels.
pub(crate) const SVG_LONG_EDGE_MIN: u32 = 1024;

/// An SVG shown in full is drawn with its long edge at most this many pixels.
pub(crate) const SVG_LONG_EDGE_MAX: u32 = 4096;

/// One picture of an animation and how long it stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The whole canvas at this step: partial frames are already composited onto the ones before.
    pub pixels: Rgba8,
    /// How long it is shown. Delays too short to see are lengthened to what browsers show.
    pub delay: MediaTime,
}

/// The frames of an animated image. A file with a single frame is a [`Decoded::Still`], so an
/// `Animation` made by this crate always has at least two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    /// The frames in order.
    pub frames: NonEmpty<Frame>,
    /// How many times the file asks to run through them.
    pub plays: Plays,
}

/// What a file decodes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// One picture, with the file's EXIF orientation already applied.
    Still(Rgba8),
    /// An animated GIF, WebP or APNG.
    Animated(Animation),
    /// The first frame of an animation whose frames together would take more than
    /// [`MAX_ANIMATION_BYTES`]: it is shown as a still and says how many it has.
    HeldStill {
        /// The first frame, whole canvas.
        picture: Rgba8,
        /// How many frames the file holds.
        frames: FrameCount,
    },
}

/// How many pictures a file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameCount(pub u32);

/// The picture or animation in the file `src` points at, whose type `sniffed` established.
/// Blocking: reads and decodes the whole file.
pub fn decode(src: &Source, sniffed: &Sniffed) -> Result<Decoded, ImageError> {
    let bytes = read(src)?;
    decode_bytes(&bytes, sniffed)
}

/// The picture or animation in `bytes`, a file whose type `sniffed` established.
///
/// **Alpha** is straight (not premultiplied); see [`Rgba8`]. **Orientation** is applied here: a
/// photo stored sideways with EXIF orientation 6 comes back upright, so the GPU layer draws the
/// pixels as given and never needs the EXIF tag. SVG is drawn at its own size, enlarged or reduced
/// into the 1024 to 4096 pixel range of its long edge.
pub fn decode_bytes(bytes: &[u8], sniffed: &Sniffed) -> Result<Decoded, ImageError> {
    let ceiling = Ceiling::VIEW;
    match codec_for(sniffed)? {
        Codec::Image(format) => match stills::animation(bytes, format, ceiling)? {
            Some(frames) => {
                match stills::collect(frames, MAX_ANIMATION_BYTES, frame_count::of(bytes, format))?
                {
                    Collected::All(frames) => animated(frames, plays_of(bytes, format)),
                    Collected::TooMany { first, count } => Ok(Decoded::HeldStill {
                        picture: first.pixels,
                        frames: FrameCount(count),
                    }),
                }
            }
            None => {
                stills::still(bytes, format, ceiling).map(|(picture, _)| Decoded::Still(picture))
            }
        },
        Codec::HighRange(format) => {
            highrange::decode(bytes, format, ceiling).map(|(picture, _)| Decoded::Still(picture))
        }
        Codec::Psd => layered::psd(bytes, ceiling).map(|(picture, _)| Decoded::Still(picture)),
        Codec::Icns => layered::icns(bytes).map(|(picture, _)| Decoded::Still(picture)),
        Codec::Jxl => jxl::decode(bytes, ceiling).map(|(picture, _)| Decoded::Still(picture)),
        Codec::RawPreview => raw::decode(bytes, ceiling).map(Decoded::Still),
        Codec::Svg => {
            let document = svg::Svg::parse(bytes)?;
            let size = svg::view_size(document.intrinsic());
            document.render(size).map(Decoded::Still)
        }
    }
}

/// The size the picture of `src` will have once decoded and turned upright, read from the file's
/// header and its EXIF orientation without decoding a pixel. `None` when the format's size is not
/// read that way (ICNS, JPEG XL and SVG, whose size comes from decoding or drawing them) or the file
/// declares more pixels than a decode accepts.
pub fn declared_size(src: &Source, sniffed: &Sniffed) -> Result<Option<PixelSize>, ImageError> {
    declared_size_of(&read(src)?, sniffed)
}

/// [`declared_size`] of a file already read.
pub(crate) fn declared_size_of(
    bytes: &[u8],
    sniffed: &Sniffed,
) -> Result<Option<PixelSize>, ImageError> {
    let (size, per_pixel) = match codec_for(sniffed)? {
        Codec::Image(format) => stills::header(bytes, format)?.cost(stills::STILL_COPIES),
        Codec::HighRange(format) => stills::header(bytes, format)?.cost(highrange::COPIES),
        Codec::Psd => layered::psd_cost(bytes)?,
        Codec::Icns | Codec::Jxl | Codec::Svg | Codec::RawPreview => return Ok(None),
    };
    if !Ceiling::VIEW.allows(size, per_pixel) {
        return Ok(None);
    }
    let turn = crate::exif::ExifFacts::read(bytes).orientation.turn;
    Ok(Some(match turn {
        QuarterTurn::None | QuarterTurn::Half => size,
        QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => PixelSize {
            width: size.height,
            height: size.width,
        },
    }))
}

/// The frames as a [`Decoded`]: a single frame is a still.
fn animated(frames: Vec<Frame>, plays: Plays) -> Result<Decoded, ImageError> {
    let Some(frames) = NonEmpty::from_vec(frames) else {
        return Err(ImageError::Decode {
            reason: "the animation has no frames".to_owned(),
        });
    };
    if frames.count().get() == 1 {
        Ok(Decoded::Still(frames.first().pixels.clone()))
    } else {
        Ok(Decoded::Animated(Animation { frames, plays }))
    }
}

/// The file's bytes.
pub(crate) fn read(src: &Source) -> Result<Vec<u8>, ImageError> {
    std::fs::read(src.path().as_path()).map_err(|e| ImageError::Read {
        path: src.path().as_path().to_path_buf(),
        kind: e.kind(),
    })
}
