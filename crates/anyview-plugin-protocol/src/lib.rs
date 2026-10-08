//! The plugin protocol, version 1 (ARCHITECTURE section 2l): what a viewer and a plugin say to
//! each other. How a message is framed on a pipe is bayonet's wire, re-exported here so a plugin's
//! main loop reaches `read_frame` and `write_frame` from the one crate. The crate has no I/O of
//! its own, and it depends on `bayonet` without its host half, `serde` and `thiserror` only, so a
//! plugin written in Rust depends on this crate and nothing of the viewer's.

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
