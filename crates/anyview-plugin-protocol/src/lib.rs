//! The plugin protocol, version 1 (ARCHITECTURE section 2l): what a viewer and a plugin say to
//! each other. How a message is framed on a pipe is bayonet's wire, re-exported here so a plugin's
//! main loop reaches `read_frame` and `write_frame` from the one crate. The crate has no I/O of
//! its own, and it depends on `bayonet` without its host half, `serde` and `thiserror` only, so a
//! plugin written in Rust depends on this crate and nothing of the viewer's.
//!
//! A plugin greets the host, and the host reads the greeting back as the same message:
//!
//! ```
//! use anyview_plugin_protocol::{
//!     Capability, Hello, PROTOCOL_VERSION, PluginMessage, WireError, read_frame, write_frame,
//! };
//!
//! let hello = PluginMessage::Hello(Hello {
//!     protocol: PROTOCOL_VERSION,
//!     name: "demo".to_owned(),
//!     provides: vec![Capability::Probe],
//!     targets: Vec::new(),
//! });
//! let mut pipe = Vec::new();
//! write_frame(&mut pipe, &hello, &[])?;
//! let frame = read_frame::<_, PluginMessage>(&mut pipe.as_slice())?;
//! assert_eq!(frame.message, hello);
//! assert!(frame.payload.is_empty());
//! # Ok::<(), WireError>(())
//! ```

#![warn(missing_docs)]

mod capability;
mod error;
mod message;

pub use bayonet::wire::{
    Frame, FrameDecoder, MAX_JSON_BYTES, MAX_PAYLOAD_BYTES, WireError, encode_frame, read_frame,
    write_frame,
};
pub use capability::Capability;
pub use error::ImageSizeError;
pub use message::{
    DecodeRequest, Done, ErrorCode, ExportRequest, FactRow, FactsReply, Failure, Hello,
    HostMessage, ImageHeader, MicroRange, PROTOCOL_VERSION, PluginMessage, ProbeRequest, Progress,
    ThumbnailRequest,
};
