//! A picture's whole adjustment written back: cut, mirrored, turned and resized, in the format
//! the file already has.
//!
//! A turn or a mirror alone is the lossless edit of the formats that have one (a JPEG keeps its
//! scan, a PNG its palette and text). A cut or a resize has to change the pixels, so the picture
//! is decoded, adjusted and encoded again in its own format, with its colour profile and EXIF
//! carried across and the orientation entry made upright, since the turn is now in the pixels.
//! Location data is carried only if the file had it.

use super::{Fidelity, Rewrite, edited, fidelity, format_of, rewrite_of};
use crate::decode::{Decoded, decode_bytes};
use crate::encode::{encode_bmp, encode_with_metadata};
use crate::error::ImageError;
use crate::orientation::{ExifOrientation, Mirror};
use crate::pixels::Rgba8;
use crate::scale::{Resampling, resampled};
use anyview_core::{
    Adjust, Axis, Edit, MetadataCarry, Percent, PixelLen, PixelRect, PixelSize, Quality,
    QuarterTurn, RasterTarget, Reflection, Sniffed,
};
use image::imageops;

/// How much of the original a re-saved JPEG keeps: high enough that a picture opened and saved
/// again looks the same.
const JPEG_QUALITY: Percent = Percent(92);

/// The file `bytes` with `adjust` applied.
pub(super) fn adjusted(
    bytes: &[u8],
    sniffed: &Sniffed,
    adjust: Adjust,
) -> Result<Vec<u8>, ImageError> {
    let format = format_of(sniffed)?;
    let how = rewrite_of(format)?;
    if fidelity(bytes, sniffed) == Fidelity::Impossible {
        return Err(ImageError::NotSavable { format });
    }
    if adjust.crop.is_none() && adjust.size.is_none() {
        return turned_only(bytes, sniffed, adjust);
    }
    let picture = adjusted_pixels(&first_picture(bytes, sniffed)?, adjust);
    match how {
        Rewrite::Orientation => encode_with_metadata(
            &picture,
            RasterTarget::Jpeg(Quality::clamped(JPEG_QUALITY)),
            bytes,
            MetadataCarry::Keep,
        ),
        Rewrite::Png => {
            encode_with_metadata(&picture, RasterTarget::Png, bytes, MetadataCarry::Keep)
        }
        Rewrite::Tiff => {
            encode_with_metadata(&picture, RasterTarget::Tiff, bytes, MetadataCarry::Keep)
        }
        Rewrite::Webp => {
            encode_with_metadata(&picture, RasterTarget::Webp, bytes, MetadataCarry::Keep)
        }
        Rewrite::Bmp => encode_bmp(&picture),
    }
}

/// A mirror and a turn with nothing cut or resized: the format's own lossless edits, one after
/// the other.
fn turned_only(bytes: &[u8], sniffed: &Sniffed, adjust: Adjust) -> Result<Vec<u8>, ImageError> {
    let mirrored = match adjust.reflection {
        Reflection::Kept => bytes.to_vec(),
        Reflection::Mirrored => edited(bytes, sniffed, Edit::Flip(Axis::Horizontal))?,
    };
    match adjust.turn {
        QuarterTurn::None => Ok(mirrored),
        QuarterTurn::Quarter | QuarterTurn::Half | QuarterTurn::ThreeQuarter => {
            edited(&mirrored, sniffed, Edit::Rotate(adjust.turn))
        }
    }
}

/// The picture of the file, upright: its first one when it holds several.
fn first_picture(bytes: &[u8], sniffed: &Sniffed) -> Result<Rgba8, ImageError> {
    Ok(match decode_bytes(bytes, sniffed)? {
        Decoded::Still(picture) | Decoded::HeldStill { picture, .. } => picture,
        Decoded::Animated(animation) => animation.frames.first().pixels.clone(),
    })
}

/// `picture` cut, then mirrored and turned, then scaled.
pub(super) fn adjusted_pixels(picture: &Rgba8, adjust: Adjust) -> Rgba8 {
    let cut = match adjust.crop {
        Some(rect) => cut_out(picture, rect),
        None => picture.clone(),
    };
    let placed = ExifOrientation {
        mirror: match adjust.reflection {
            Reflection::Kept => Mirror::Unmirrored,
            Reflection::Mirrored => Mirror::Mirrored,
        },
        turn: adjust.turn,
    }
    .applied(&cut);
    match adjust.size {
        Some(size) if size != placed.size() => {
            resampled(&placed, at_least_one(size), Resampling::Sharp)
        }
        Some(_) | None => placed,
    }
}

fn at_least_one(size: PixelSize) -> PixelSize {
    PixelSize {
        width: PixelLen(size.width.0.max(1)),
        height: PixelLen(size.height.0.max(1)),
    }
}

/// The part of `picture` in `rect`, held inside the picture and at least a pixel each way.
fn cut_out(picture: &Rgba8, rect: PixelRect) -> Rgba8 {
    let size = picture.size();
    let left = rect.left.0.min(size.width.0.saturating_sub(1));
    let top = rect.top.0.min(size.height.0.saturating_sub(1));
    let width = rect.size.width.0.clamp(1, (size.width.0 - left).max(1));
    let height = rect.size.height.0.clamp(1, (size.height.0 - top).max(1));
    match picture.to_image() {
        Some(image) => {
            Rgba8::from_image(imageops::crop_imm(&image, left, top, width, height).to_image())
        }
        None => picture.clone(), // a buffer `Rgba8::new` accepted always fits its size
    }
}
