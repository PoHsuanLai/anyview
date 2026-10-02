//! Decoding: a file becomes straight RGBA8 pixels, upright, as one picture or as the frames of an
//! animation.
//!
//! Everything here is blocking and runs on the caller's worker. Nothing reads a clock or spawns.

mod codec;
mod colour;
mod jxl;
mod look;
mod stills;
mod svg;

pub use colour::{ColourInfo, ColourModel};
pub(crate) use look::{Looked, look};

use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::{MediaTime, NonEmpty, PixelArea, PixelSize, QuarterTurn, Sniffed, Source};
use codec::{Codec, codec_for};

/// The most pixels a file may declare before it is refused instead of decoded: 16384 by 16384, a
/// gibibyte of RGBA. A header that claims more is a decompression bomb or not a photograph.
pub(crate) const MAX_DECODE_AREA: PixelArea = PixelArea(268_435_456);

/// The most memory the frames of one animation may take together.
pub(crate) const MAX_ANIMATION_BYTES: u64 = 512 * 1024 * 1024;

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
/// `Animation` made by this crate always has at least two. It loops forever: the container's loop
/// count is not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    /// The frames in order.
    pub frames: NonEmpty<Frame>,
}

/// What a file decodes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// One picture, with the file's EXIF orientation already applied.
    Still(Rgba8),
    /// An animated GIF, WebP or APNG.
    Animated(Animation),
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
    match codec_for(sniffed)? {
        Codec::Image(format) => match stills::animation(bytes, format)? {
            Some(frames) => animated(stills::collect(frames)?),
            None => stills::still(bytes, format).map(|(picture, _)| Decoded::Still(picture)),
        },
        Codec::Jxl => jxl::decode(bytes).map(|(picture, _)| Decoded::Still(picture)),
        Codec::Svg => {
            let document = svg::Svg::parse(bytes)?;
            let size = svg::view_size(document.intrinsic());
            document.render(size).map(Decoded::Still)
        }
    }
}

/// The size the picture of `src` will have once decoded and turned upright, read from the file's
/// header and its EXIF orientation without decoding a pixel. `None` when the format's size is not
/// read that way (JPEG XL and SVG, whose size comes from decoding or drawing them) or the file
/// declares more pixels than a decode accepts.
pub fn declared_size(src: &Source, sniffed: &Sniffed) -> Result<Option<PixelSize>, ImageError> {
    let Codec::Image(format) = codec_for(sniffed)? else {
        return Ok(None);
    };
    let bytes = read(src)?;
    let size = stills::declared_size(&bytes, format)?;
    if size.area() > MAX_DECODE_AREA {
        return Ok(None);
    }
    let turn = crate::exif::ExifFacts::read(&bytes).orientation.turn;
    Ok(Some(match turn {
        QuarterTurn::None | QuarterTurn::Half => size,
        QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => PixelSize {
            width: size.height,
            height: size.width,
        },
    }))
}

/// The frames as a [`Decoded`]: a single frame is a still.
fn animated(frames: Vec<Frame>) -> Result<Decoded, ImageError> {
    let Some(frames) = NonEmpty::from_vec(frames) else {
        return Err(ImageError::Decode {
            reason: "the animation has no frames".to_owned(),
        });
    };
    if frames.count().get() == 1 {
        Ok(Decoded::Still(frames.first().pixels.clone()))
    } else {
        Ok(Decoded::Animated(Animation { frames }))
    }
}

/// The file's bytes.
pub(crate) fn read(src: &Source) -> Result<Vec<u8>, ImageError> {
    std::fs::read(src.path().as_path()).map_err(|e| ImageError::Read {
        path: src.path().as_path().to_path_buf(),
        kind: e.kind(),
    })
}
