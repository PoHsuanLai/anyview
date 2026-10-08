//! The one error the protocol crate returns of its own: the framing's errors are bayonet's
//! `WireError`.

/// Pixels announced by an image header do not match the payload.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a {width} x {height} image needs {wanted} bytes, the payload has {have}")]
pub struct ImageSizeError {
    /// Announced width.
    pub width: u32,
    /// Announced height.
    pub height: u32,
    /// Four bytes a pixel.
    pub wanted: u64,
    /// The payload's length.
    pub have: u64,
}
