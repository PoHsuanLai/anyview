//! The one error `anyview-image` returns. Each variant is something a caller acts on: a missing
//! file is told apart from a file that is not the image its name says, a format this build cannot
//! open from one nobody can.

use anyview_core::{FormatKind, PixelSize, RasterFormat};
use std::path::PathBuf;

/// Why an image could not be read, shown, written or rotated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// The disk refused to read the file.
    #[error("cannot read {path:?}: {kind}")]
    Read {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The file is not an image: its sniffed kind belongs to another crate.
    #[error("a {kind:?} file is not an image this crate opens")]
    WrongKind {
        /// What the file was sniffed as.
        kind: FormatKind,
    },
    /// The format is an image format but has no decoder in the viewer.
    #[error("no decoder for {format:?} images")]
    Unsupported {
        /// The format.
        format: RasterFormat,
    },
    /// The format has a decoder that this build left out (AVIF needs the `avif` feature and the
    /// dav1d library).
    #[error("{format:?} images need a feature this build does not have")]
    NotCompiledIn {
        /// The format.
        format: RasterFormat,
    },
    /// The bytes are not a valid image of the format they claim.
    #[error("cannot decode the image: {reason}")]
    Decode {
        /// The decoder's own words.
        reason: String,
    },
    /// The image is larger than the decoder may allocate for.
    #[error("a {}x{} image is too large to decode", .size.width.0, .size.height.0)]
    TooLarge {
        /// The size the file declares.
        size: PixelSize,
    },
    /// A peek budget that allows no pixel.
    #[error("the peek budget allows no pixels")]
    NoBudget,
    /// A pixel buffer whose length is not four bytes per pixel.
    #[error("{len} bytes are not a {}x{} RGBA picture", .size.width.0, .size.height.0)]
    PixelsMismatch {
        /// The size claimed.
        size: PixelSize,
        /// The bytes given.
        len: usize,
    },
    /// An encoder failed.
    #[error("cannot encode the image: {reason}")]
    Encode {
        /// The encoder's own words.
        reason: String,
    },
    /// The file is not a container metadata can be spliced into or read from.
    #[error("not a JPEG, PNG or WebP container: {reason}")]
    Container {
        /// What the container parser said.
        reason: String,
    },
    /// An EXIF block that cannot be read or patched.
    #[error("cannot patch the EXIF block: {reason}")]
    Exif {
        /// What is wrong with it.
        reason: &'static str,
    },
}
