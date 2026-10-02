//! The shared thumbnail cache: a file's small picture, made once by whoever saw it first and
//! reused by every program that follows the freedesktop thumbnail spec.

use crate::error::PlatformError;
use anyview_core::{FilePath, FileStamp, PixelLen, PixelSize};
use ds_core::word::Word;

/// The spec's sizes, named by the directory they live in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ThumbSize {
    /// 128 pixels on the long edge.
    Normal,
    /// 256 pixels on the long edge.
    Large,
    /// 512 pixels on the long edge.
    XLarge,
}

impl ThumbSize {
    /// The longest edge a thumbnail of this size may have.
    pub fn edge(self) -> PixelLen {
        match self {
            ThumbSize::Normal => PixelLen(128),
            ThumbSize::Large => PixelLen(256),
            ThumbSize::XLarge => PixelLen(512),
        }
    }

    /// The directory under `thumbnails/` that holds this size.
    pub(crate) fn directory(self) -> &'static str {
        match self {
            ThumbSize::Normal => "normal",
            ThumbSize::Large => "large",
            ThumbSize::XLarge => "x-large",
        }
    }
}

/// A thumbnail's pixels, straight RGBA8 in rows from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbPixels {
    size: PixelSize,
    rgba: Vec<u8>,
}

impl ThumbPixels {
    /// `rgba` as a picture of `size`, or `None` when it is not four bytes per pixel or the
    /// picture is empty.
    pub fn new(size: PixelSize, rgba: Vec<u8>) -> Option<ThumbPixels> {
        let wanted = size.area().0.checked_mul(4)?;
        let fits = u64::try_from(rgba.len()).is_ok_and(|len| len == wanted);
        (fits && size.area().0 > 0).then_some(ThumbPixels { size, rgba })
    }

    /// Width and height.
    pub fn size(&self) -> PixelSize {
        self.size
    }

    /// The pixels, four bytes each.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// Read and write the shared thumbnails.
pub trait ThumbnailCache {
    /// The thumbnail of `file` as it was at `stamp`, or `None` when there is none, or it was
    /// made for a different version of the file, or it is damaged. Blocking.
    fn lookup(
        &self,
        file: &FilePath,
        stamp: &FileStamp,
        size: ThumbSize,
    ) -> Result<Option<ThumbPixels>, PlatformError>;

    /// Keep `pixels` as the thumbnail of `file` at `stamp`, replacing any earlier one. Blocking.
    fn store(
        &self,
        file: &FilePath,
        stamp: &FileStamp,
        size: ThumbSize,
        pixels: &ThumbPixels,
    ) -> Result<(), PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_must_be_four_bytes_a_pixel_and_not_empty() {
        let size = |w, h| PixelSize {
            width: PixelLen(w),
            height: PixelLen(h),
        };
        const CASES: &[(&str, u32, u32, usize, bool)] = &[
            ("exact", 2, 1, 8, true),
            ("short", 2, 1, 7, false),
            ("long", 2, 1, 9, false),
            ("empty", 0, 1, 0, false),
        ];
        for (name, w, h, len, valid) in CASES {
            assert_eq!(
                ThumbPixels::new(size(*w, *h), vec![0; *len]).is_some(),
                *valid,
                "{name}"
            );
        }
    }
}
