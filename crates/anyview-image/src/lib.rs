//! Images for the viewer: decode, a downscaled peek, EXIF facts and orientation, encode for
//! export, and lossless JPEG rotation. Blocking and pure of the runtime: every function runs on
//! the caller's worker, and none spawns or reads a clock.
//!
//! Every public item is reached from this root, once.

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
