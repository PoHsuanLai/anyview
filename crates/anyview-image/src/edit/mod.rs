//! An edit applied to a picture file: the bytes of the file it becomes, and which edits a file
//! takes at all.
//!
//! The promise is that a saved file keeps everything a person would notice. A JPEG is never
//! decoded: its EXIF orientation entry changes and every other byte stays. PNG, TIFF, lossless
//! WebP and BMP are moved sample by sample at the depth and colour type they have, and keep their
//! palette, pages, text, resolution and colour profile. A file that cannot be written back that
//! way is not edited: [`editable`] says so before anything is offered.

mod bmp;
mod place;
mod png;
mod tiff;
mod webp;

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

/// What the person asked for: a clockwise turn or a mirror.
#[derive(Debug, Clone, Copy)]
struct Placing {
    turn: QuarterTurn,
    axis: Option<Axis>,
}

impl Placing {
    /// The orientation of a picture stored as `current` after this is done to what is shown.
    fn over(self, current: ExifOrientation) -> ExifOrientation {
        match self.axis {
            Some(axis) => current.flipped(axis),
            None => current.turned(self.turn),
        }
    }

    /// Whether width and height trade places.
    fn swaps(self) -> bool {
        self.axis.is_none() && matches!(self.turn, QuarterTurn::Quarter | QuarterTurn::ThreeQuarter)
    }
}

/// How a format is written back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rewrite {
    /// Change the orientation entry; touch nothing else.
    Orientation,
    Png,
    Tiff,
    Webp,
    Bmp,
}

/// How `format` is saved in place, or why it cannot be: a format with no writer here that keeps
/// it as it is would lose something the person did not ask to lose.
fn rewrite_of(format: RasterFormat) -> Result<Rewrite, ImageError> {
    match format {
        RasterFormat::Jpeg => Ok(Rewrite::Orientation),
        RasterFormat::Png => Ok(Rewrite::Png),
        RasterFormat::Webp => Ok(Rewrite::Webp),
        RasterFormat::Tiff => Ok(Rewrite::Tiff),
        RasterFormat::Bmp => Ok(Rewrite::Bmp),
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

fn format_of(sniffed: &Sniffed) -> Result<RasterFormat, ImageError> {
    match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Raster, FormatDetail::Raster(format)) => Ok(*format),
        (kind, _) => Err(ImageError::WrongKind { kind }),
    }
}

/// What a person loses by an edit, in the words a dialog would use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Loss {
    /// A multi-page file would keep one page.
    Pages,
    /// An animation would keep one frame.
    Animation,
    /// The picture would be saved with less colour detail.
    Colour,
    /// Information stored beside the picture would not be kept.
    Details,
}

impl Loss {
    /// One plain sentence for what is lost.
    pub fn sentence(self) -> &'static str {
        match self {
            Loss::Pages => "Only the first page will be kept.",
            Loss::Animation => "Only the first frame will be kept.",
            Loss::Colour => "Some colour detail may be lost.",
            Loss::Details => "Some information stored in the file may be dropped.",
        }
    }
}

/// What turning or flipping a file costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fidelity {
    /// Everything is kept: nothing to ask.
    Intact,
    /// The edit can be written, and loses this.
    Loses(Loss),
    /// No valid file can be written: the edit is not offered.
    Impossible,
}

/// What rotating or flipping the picture file `bytes` costs; both edits cost the same. Reads
/// headers and chunk lists, never pixels.
pub fn fidelity(bytes: &[u8], sniffed: &Sniffed) -> Fidelity {
    match format_of(sniffed).and_then(rewrite_of) {
        Ok(Rewrite::Orientation) => Fidelity::Intact,
        Ok(Rewrite::Png) => png::fidelity(bytes),
        Ok(Rewrite::Tiff) => tiff::fidelity(bytes),
        Ok(Rewrite::Webp) => webp::fidelity(bytes),
        Ok(Rewrite::Bmp) => bmp::fidelity(bytes),
        Err(_) => Fidelity::Impossible,
    }
}

/// The file `bytes`, which `sniffed` says is a picture, with `edit` applied, keeping what
/// [`fidelity`] says it keeps: a file that loses something is written anyway (the caller asked
/// first). Blocking: a re-encoded format is decoded and written whole. An edit of pages, a vector
/// picture and a format with no writer are refused.
pub fn edited(bytes: &[u8], sniffed: &Sniffed, edit: Edit) -> Result<Vec<u8>, ImageError> {
    let placing = match edit {
        Edit::Rotate(turn) => Placing { turn, axis: None },
        Edit::Flip(axis) => Placing {
            turn: QuarterTurn::None,
            axis: Some(axis),
        },
        Edit::DeletePages(_) | Edit::MovePage { .. } => {
            return Err(ImageError::NotAnImageEdit { kind: edit.kind() });
        }
    };
    let format = format_of(sniffed)?;
    let how = rewrite_of(format)?;
    let seen = fidelity(bytes, sniffed);
    if seen == Fidelity::Impossible {
        return Err(ImageError::NotSavable { format });
    }
    let flat = matches!(
        seen,
        Fidelity::Loses(Loss::Pages | Loss::Colour | Loss::Animation)
    );
    match how {
        Rewrite::Orientation => match placing.axis {
            Some(axis) => flip_jpeg(bytes, axis),
            None => rotate_jpeg(bytes, placing.turn),
        },
        Rewrite::Png => png::rewritten(bytes, placing),
        Rewrite::Tiff if !flat => tiff::rewritten(bytes, placing),
        Rewrite::Tiff => encode_with_metadata(
            &moved_pixels(bytes, sniffed, placing)?,
            RasterTarget::Tiff,
            bytes,
            MetadataCarry::Keep,
        ),
        Rewrite::Webp => webp::rewritten(bytes, &moved_pixels(bytes, sniffed, placing)?),
        Rewrite::Bmp if !flat => {
            bmp::rewritten(bytes, placing, &moved_pixels(bytes, sniffed, placing)?)
        }
        Rewrite::Bmp => encode_bmp(&moved_pixels(bytes, sniffed, placing)?),
    }
}

/// The picture of `bytes` turned or mirrored.
fn moved_pixels(bytes: &[u8], sniffed: &Sniffed, placing: Placing) -> Result<Rgba8, ImageError> {
    let picture = match decode_bytes(bytes, sniffed)? {
        Decoded::Still(picture) | Decoded::HeldStill { picture, .. } => picture,
        Decoded::Animated(animation) => animation.frames.first().pixels.clone(),
    };
    let by = placing.over(ExifOrientation {
        mirror: Mirror::Unmirrored,
        turn: QuarterTurn::None,
    });
    Ok(by.applied(&picture))
}
