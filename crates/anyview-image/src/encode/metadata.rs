//! Carrying EXIF and ICC across a re-encode with `img-parts`: JPEG, PNG and WebP hold them as
//! separate segments, so they are spliced into the freshly encoded file without touching pixels.

use crate::error::ImageError;
use crate::exif::with_orientation;
use crate::orientation::ExifOrientation;
use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};

/// The EXIF block (a raw TIFF) of `original` with its orientation reset to upright, if it has one
/// that can be reset. A block that cannot be patched is not carried: writing it would turn the
/// already-upright pixels a second time.
pub(super) fn exif_of(original: &[u8]) -> Option<Vec<u8>> {
    let image = DynImage::from_bytes(Bytes::copy_from_slice(original)).ok()??;
    let exif = image.exif()?;
    with_orientation(&exif, ExifOrientation::UPRIGHT).ok()
}

/// `encoded` (a JPEG, PNG or WebP file) with the EXIF block and ICC profile of `original` added.
/// An `original` that is none of those, or has neither, returns `encoded` as it was; so does an
/// `encoded` that is none of those (TIFF holds its metadata inside its own structure).
pub(super) fn carry_metadata(encoded: Vec<u8>, original: &[u8]) -> Result<Vec<u8>, ImageError> {
    let icc = DynImage::from_bytes(Bytes::copy_from_slice(original))
        .ok()
        .flatten()
        .and_then(|image| image.icc_profile());
    let exif = exif_of(original);
    if icc.is_none() && exif.is_none() {
        return Ok(encoded);
    }
    let container = |e: img_parts::Error| ImageError::Container {
        reason: e.to_string(),
    };
    let Some(mut image) = DynImage::from_bytes(Bytes::from(encoded.clone())).map_err(container)?
    else {
        return Ok(encoded);
    };
    if let Some(icc) = icc {
        image.set_icc_profile(Some(icc));
    }
    if let Some(exif) = exif {
        image.set_exif(Some(Bytes::from(exif)));
    }
    Ok(image.encoder().bytes().to_vec())
}
