//! Carrying EXIF and ICC across a re-encode with `img-parts`: JPEG, PNG and WebP hold them as
//! separate segments, so they are spliced into the freshly encoded file without touching pixels.

use crate::error::ImageError;
use crate::exif::{with_orientation, without_location};
use crate::orientation::ExifOrientation;
use anyview_core::MetadataCarry;
use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};

/// What of an original a re-encode carries: its colour profile always, its EXIF as `keep` allows.
#[derive(Debug, Default)]
pub(super) struct Carried {
    /// The ICC profile, which the pixels are meaningless without.
    pub(super) icc: Option<Bytes>,
    /// The EXIF block (a raw TIFF), upright, with or without where the picture was taken.
    pub(super) exif: Option<Vec<u8>>,
}

impl Carried {
    /// What `keep` says to carry of `original`. An original that is not a container with
    /// separate segments (TIFF, AVIF, an unknown file) carries nothing.
    pub(super) fn of(original: &[u8], keep: MetadataCarry) -> Self {
        let Some(image) = DynImage::from_bytes(Bytes::copy_from_slice(original))
            .ok()
            .flatten()
        else {
            return Self::default();
        };
        // An EXIF block that cannot be set upright is not carried: writing it would turn the
        // already-upright pixels a second time.
        let exif = match keep {
            MetadataCarry::Drop => None,
            MetadataCarry::Keep | MetadataCarry::StripLocation => image
                .exif()
                .and_then(|exif| with_orientation(&exif, ExifOrientation::UPRIGHT).ok())
                .map(|block| match keep {
                    MetadataCarry::StripLocation => without_location(&block),
                    MetadataCarry::Keep | MetadataCarry::Drop => block,
                }),
        };
        Carried {
            icc: image.icc_profile(),
            exif,
        }
    }

    /// `encoded` (a JPEG, PNG or WebP file) with this added. A file that is none of those (TIFF
    /// holds its metadata inside its own structure), or nothing to add, returns it as it was.
    pub(super) fn into(self, encoded: Vec<u8>) -> Result<Vec<u8>, ImageError> {
        if self.icc.is_none() && self.exif.is_none() {
            return Ok(encoded);
        }
        let container = |e: img_parts::Error| ImageError::Container {
            reason: e.to_string(),
        };
        let Some(mut image) =
            DynImage::from_bytes(Bytes::from(encoded.clone())).map_err(container)?
        else {
            return Ok(encoded);
        };
        if let Some(icc) = self.icc {
            image.set_icc_profile(Some(icc));
        }
        if let Some(exif) = self.exif {
            image.set_exif(Some(Bytes::from(exif)));
        }
        Ok(image.encoder().bytes().to_vec())
    }
}
