//! Encoding: pixels become the file an export writes.
//!
//! Input pixels are upright straight RGBA8, as [`crate::decode`] returns them. Metadata is kept
//! only for the containers that carry it as separate segments (JPEG, PNG, WebP, and EXIF in AVIF).

mod avif;
mod codecs;
mod metadata;

use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::{MetadataCarry, RasterTarget};

/// `picture` encoded as `target`: PNG; JPEG at its quality (alpha is composited onto white, since
/// JPEG has none); lossless WebP; AVIF at its quality; or TIFF. No metadata is written.
///
/// AVIF is slow on large pictures: run it on a worker.
pub fn encode(picture: &Rgba8, target: RasterTarget) -> Result<Vec<u8>, ImageError> {
    match target {
        RasterTarget::Png => codecs::png(picture),
        RasterTarget::Jpeg(quality) => codecs::jpeg(picture, quality),
        RasterTarget::Webp => codecs::webp(picture),
        RasterTarget::Avif(quality) => avif::encode(picture, quality, None),
        RasterTarget::Tiff => codecs::tiff(picture),
    }
}

/// `picture` as an uncompressed Windows bitmap. `RasterTarget` has no BMP entry because the
/// export sheet does not offer one; the conversion exists for callers that need the format.
pub fn encode_bmp(picture: &Rgba8) -> Result<Vec<u8>, ImageError> {
    codecs::bmp(picture)
}

/// `picture` encoded as `target`, with the EXIF block and ICC profile of `original` (the file the
/// picture was decoded from) carried across when `keep` says so and the container can hold them.
/// The carried EXIF has its orientation reset to upright, because the pixels already are.
pub fn encode_with_metadata(
    picture: &Rgba8,
    target: RasterTarget,
    original: &[u8],
    keep: MetadataCarry,
) -> Result<Vec<u8>, ImageError> {
    match keep {
        MetadataCarry::Drop => encode(picture, target),
        MetadataCarry::Keep => match target {
            RasterTarget::Avif(quality) => {
                let exif = metadata::exif_of(original);
                avif::encode(picture, quality, exif.as_deref())
            }
            RasterTarget::Png | RasterTarget::Jpeg(_) | RasterTarget::Webp | RasterTarget::Tiff => {
                metadata::carry_metadata(encode(picture, target)?, original)
            }
        },
    }
}
