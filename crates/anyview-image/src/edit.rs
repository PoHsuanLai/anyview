//! An edit applied to a picture file: the bytes of the file it becomes. A JPEG is never decoded:
//! its EXIF orientation entry changes and every other byte stays. The other formats the viewer
//! writes without loss are decoded, turned or mirrored, and written again in the same format
//! with their EXIF block and colour profile.

use crate::decode::{Decoded, decode_bytes};
use crate::encode::{encode_bmp, encode_with_metadata};
use crate::error::ImageError;
use crate::orientation::{ExifOrientation, Mirror};
use crate::pixels::Rgba8;
use crate::rotate::{flip_jpeg, rotate_jpeg};
use anyview_core::{
    Axis, Edit, FormatDetail, FormatKind, MetadataCarry, QuarterTurn, RasterFormat, RasterTarget,
    Sniffed,
};

/// How a format is written back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rewrite {
    /// Change the orientation entry; touch nothing else.
    Orientation,
    /// Write the pixels again, losslessly, in this format with the original's metadata.
    Pixels(RasterTarget),
    /// Write the pixels again as a bitmap, which carries no metadata.
    Bitmap,
}

/// How `format` is saved in place, or why it cannot be: a format with no lossless writer here,
/// or a lossy one, would lose something the person did not ask to lose.
fn rewrite_of(format: RasterFormat) -> Result<Rewrite, ImageError> {
    match format {
        RasterFormat::Jpeg => Ok(Rewrite::Orientation),
        RasterFormat::Png => Ok(Rewrite::Pixels(RasterTarget::Png)),
        RasterFormat::Webp => Ok(Rewrite::Pixels(RasterTarget::Webp)),
        RasterFormat::Tiff => Ok(Rewrite::Pixels(RasterTarget::Tiff)),
        RasterFormat::Bmp => Ok(Rewrite::Bitmap),
        RasterFormat::Gif
        | RasterFormat::Ico
        | RasterFormat::Tga
        | RasterFormat::Qoi
        | RasterFormat::Avif
        | RasterFormat::Jxl
        | RasterFormat::Heic
        | RasterFormat::Psd
        | RasterFormat::Icns
        | RasterFormat::Exr
        | RasterFormat::Hdr
        | RasterFormat::Raw => Err(ImageError::NotSavable { format }),
    }
}

/// The file `bytes`, which `sniffed` says is a picture, with `edit` applied. Blocking: a
/// re-encoded format is decoded and written whole. An edit of pages, a vector picture, an
/// animation and a format with no lossless writer are refused.
pub fn edited(bytes: &[u8], sniffed: &Sniffed, edit: Edit) -> Result<Vec<u8>, ImageError> {
    let (turn, axis) = match edit {
        Edit::Rotate(turn) => (turn, None),
        Edit::Flip(axis) => (QuarterTurn::None, Some(axis)),
        Edit::DeletePages(_) | Edit::MovePage { .. } => {
            return Err(ImageError::NotAnImageEdit { kind: edit.kind() });
        }
    };
    let format = match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Raster, FormatDetail::Raster(format)) => *format,
        (kind, _) => return Err(ImageError::WrongKind { kind }),
    };
    match rewrite_of(format)? {
        Rewrite::Orientation => match axis {
            Some(axis) => flip_jpeg(bytes, axis),
            None => rotate_jpeg(bytes, turn),
        },
        Rewrite::Pixels(target) => {
            let moved = moved_pixels(bytes, sniffed, turn, axis)?;
            encode_with_metadata(&moved, target, bytes, MetadataCarry::Keep)
        }
        Rewrite::Bitmap => encode_bmp(&moved_pixels(bytes, sniffed, turn, axis)?),
    }
}

/// The picture of `bytes` turned by `turn`, or mirrored across `axis`.
fn moved_pixels(
    bytes: &[u8],
    sniffed: &Sniffed,
    turn: QuarterTurn,
    axis: Option<Axis>,
) -> Result<Rgba8, ImageError> {
    let Decoded::Still(picture) = decode_bytes(bytes, sniffed)? else {
        return Err(ImageError::NotSavableAnimated);
    };
    let placing = match axis {
        Some(axis) => ExifOrientation::UPRIGHT.flipped(axis),
        None => ExifOrientation {
            mirror: Mirror::Unmirrored,
            turn,
        },
    };
    Ok(placing.applied(&picture))
}
