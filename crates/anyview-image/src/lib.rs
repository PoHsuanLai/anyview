//! Images for the viewer: decode, a downscaled peek, EXIF facts and orientation, encode for
//! export, and lossless JPEG rotation. Blocking and pure of the runtime: every function runs on
//! the caller's worker, and none spawns or reads a clock.
//!
//! Every public item is reached from this root, once.
//!
//! The front door is [`decode_bytes`]: the host sniffs the file's head and name with
//! `anyview_core::sniff`, then hands the bytes and what it found here.
//!
//! ```
//! use anyview_core::{FileHead, FileName, PixelLen, PixelSize, SniffStep, sniff};
//! use anyview_image::{Decoded, Rgba8, decode_bytes, encode_png};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // A picture of two pixels, one red and one blue, written as PNG bytes in memory.
//! let size = PixelSize {
//!     width: PixelLen(2),
//!     height: PixelLen(1),
//! };
//! let picture = Rgba8::new(size, vec![255, 0, 0, 255, 0, 0, 255, 255])?;
//! let png = encode_png(&picture)?;
//!
//! // The host sniffs the head and the name, then decodes with what it found.
//! let name = FileName::new("pixels.png")?;
//! let SniffStep::Done(sniffed) = sniff(&FileHead::new(&png), &name) else {
//!     panic!("a PNG is not a zip");
//! };
//! match decode_bytes(&png, &sniffed)? {
//!     Decoded::Still(decoded) => assert_eq!(decoded, picture),
//!     other => panic!("expected one still picture, got {other:?}"),
//! }
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod decode;
#[cfg(feature = "encode")]
mod edit;
#[cfg(feature = "encode")]
mod encode;
mod error;
mod exif;
#[cfg(feature = "encode")]
mod export;
mod orientation;
mod peek;
mod picture_facts;
mod pixels;
mod png;
mod resolution;
mod rotate;
mod scale;

pub use decode::{
    Animation, ColourInfo, ColourModel, Decoded, Frame, FrameCount, Plays, colour_of,
    declared_size, decode, decode_bytes, file_bytes, natural_size,
};
#[cfg(feature = "encode")]
pub use edit::{Fidelity, Loss, edited, fidelity};
#[cfg(feature = "encode")]
pub use encode::{encode, encode_bmp, encode_with_metadata};
pub use error::ImageError;
pub use exif::{ExifFacts, Exposure, Flash, FlashMode, FlashState, Location, Ratio, SignedRatio};
#[cfg(feature = "encode")]
pub use export::{ImageFile, encode_file, plan_export};
pub use orientation::{ExifOrientation, Mirror};
pub use peek::{ImagePeek, PeekedFormat, RasterPeek, VectorPeek};
pub use picture_facts::picture_facts;
pub use pixels::{PremultipliedRgba8, Rgba8};
pub use png::encode_png;
pub use resolution::Resolution;
pub use rotate::{flip_jpeg, rotate_jpeg};
pub use scale::resized;
