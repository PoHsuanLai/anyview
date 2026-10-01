//! Images for the viewer: decode, a downscaled peek, EXIF facts and orientation, encode for
//! export, and lossless JPEG rotation. Blocking and pure of the runtime: every function runs on
//! the caller's worker, and none spawns or reads a clock.
//!
//! Every public item is reached from this root, once.

mod decode;
mod encode;
mod error;
mod exif;
mod orientation;
mod peek;
mod pixels;
mod scale;

pub use decode::{
    Animation, ColourInfo, ColourModel, Decoded, Frame, FrameCount, decode, decode_bytes,
};
pub use encode::{encode, encode_bmp, encode_with_metadata};
pub use error::ImageError;
pub use exif::{ExifFacts, Exposure, Ratio};
pub use orientation::{ExifOrientation, Mirror};
pub use peek::{ImagePeek, PeekedFormat, RasterPeek, VectorPeek};
pub use pixels::{PremultipliedRgba8, Rgba8};
pub use scale::resized;
