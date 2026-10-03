//! The one error the protocol crate returns.

/// Why bytes are not a message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// A frame announces more JSON than a message may carry.
    #[error("a message of {len} bytes exceeds the limit of {limit}")]
    JsonTooLarge {
        /// What the header announced.
        len: u64,
        /// The limit.
        limit: u64,
    },
    /// A frame announces more payload than a message may carry.
    #[error("a payload of {len} bytes exceeds the limit of {limit}")]
    PayloadTooLarge {
        /// What the header announced.
        len: u64,
        /// The limit.
        limit: u64,
    },
    /// The JSON is not a message of the expected type.
    #[error("not a valid message: {reason}")]
    Malformed {
        /// What the JSON codec reported.
        reason: String,
    },
    /// A message cannot be written as JSON (a path that is not UTF-8).
    #[error("cannot write the message: {reason}")]
    Unwritable {
        /// What the JSON codec reported.
        reason: String,
    },
    /// The stream ended inside a frame.
    #[error("the stream ended inside a frame")]
    Truncated,
    /// The stream ended between frames: the other side closed it.
    #[error("the stream ended")]
    Closed,
    /// Reading or writing the stream failed.
    #[error("the stream failed: {kind}")]
    Io {
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// Pixels announced by an image header do not match the payload.
    #[error("a {width} x {height} image needs {wanted} bytes, the payload has {have}")]
    ImageSize {
        /// Announced width.
        width: u32,
        /// Announced height.
        height: u32,
        /// Four bytes a pixel.
        wanted: u64,
        /// The payload's length.
        have: u64,
    },
}
