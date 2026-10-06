//! How much memory one decode may hold at its peak, and the check that a header's claim fits it.
//!
//! A decode holds more than the picture it returns: the codec's own buffer in the file's colour
//! depth, the RGBA8 copy, the copy that turns it upright. A size that fits a gibibyte of RGBA8 can
//! need three times that at its peak, so the limit is on the peak, worked out from the size and
//! what each pixel costs the whole decode, before any pixel is decoded.

use crate::error::ImageError;
use anyview_core::PixelSize;

/// The peak memory a decode may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ceiling(u64);

impl Ceiling {
    /// What the viewer allows for a file it opens: about 130 megapixels of an 8-bit photograph.
    pub(crate) const VIEW: Ceiling = Ceiling(1536 * 1024 * 1024);

    /// What a peek allows, which only needs a small picture: about 70 megapixels of an 8-bit
    /// photograph. The launcher decodes on one worker, so a peek stays well under the viewer.
    pub(crate) const PEEK: Ceiling = Ceiling(768 * 1024 * 1024);

    /// The most the decoder itself may allocate, for the `image` crate's own limit.
    pub(crate) fn bytes(self) -> u64 {
        self.0
    }

    /// Whether a decode of `size` that costs `bytes_per_pixel` at its peak fits.
    pub(crate) fn allows(self, size: PixelSize, bytes_per_pixel: u64) -> bool {
        size.area()
            .0
            .checked_mul(bytes_per_pixel.max(1))
            .is_some_and(|peak| peak <= self.0)
    }

    /// `Ok` when a decode of `size` that costs `bytes_per_pixel` at its peak fits, else the
    /// picture is too large.
    pub(crate) fn admit(self, size: PixelSize, bytes_per_pixel: u64) -> Result<(), ImageError> {
        if self.allows(size, bytes_per_pixel) {
            Ok(())
        } else {
            Err(ImageError::TooLarge { size })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::PixelLen;

    fn size(width: u32, height: u32) -> PixelSize {
        PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        }
    }

    #[test]
    fn the_peak_decides_not_the_area_alone() {
        // name, ceiling, size, bytes per pixel, fits
        const CASES: &[(&str, Ceiling, (u32, u32), u64, bool)] = &[
            (
                "a photo in the viewer",
                Ceiling::VIEW,
                (8000, 6000),
                11,
                true,
            ),
            (
                "a 16k square rgb photo in the viewer",
                Ceiling::VIEW,
                (16384, 16384),
                11,
                false,
            ),
            (
                "the same photo as a rgba8 in the viewer",
                Ceiling::VIEW,
                (8000, 6000),
                12,
                true,
            ),
            (
                "a 100 megapixel photo in a peek",
                Ceiling::PEEK,
                (11648, 8736),
                11,
                false,
            ),
            (
                "a 50 megapixel photo in a peek",
                Ceiling::PEEK,
                (8000, 6250),
                11,
                true,
            ),
            (
                "the same area costs more at 16 bits",
                Ceiling::PEEK,
                (8000, 6250),
                20,
                false,
            ),
            (
                "a claim that overflows",
                Ceiling::VIEW,
                (u32::MAX, u32::MAX),
                u64::MAX,
                false,
            ),
            ("a zero cost counts as one", Ceiling::PEEK, (1, 1), 0, true),
        ];
        for (name, ceiling, (w, h), per_pixel, fits) in CASES {
            assert_eq!(ceiling.allows(size(*w, *h), *per_pixel), *fits, "{name}");
        }
    }
}
