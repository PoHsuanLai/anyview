//! Text for the viewer.

mod bytes;
mod encoding;
mod error;
mod lines;

pub use bytes::{ByteSource, FileBytes, HeldBytes};
pub use encoding::{Coverage, DETECT_BYTES, Detected, TextCodec, detect};
pub use error::TextError;
pub use lines::{LineCount, TextLines};
