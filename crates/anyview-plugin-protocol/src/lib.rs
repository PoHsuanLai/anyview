//! The plugin protocol, version 1 (ARCHITECTURE section 2l): what a viewer and a plugin say to
//! each other and how a message is framed on a pipe. The crate has no I/O of its own beyond the
//! blocking `read_frame` and `write_frame` a plugin's main loop uses over its standard streams,
//! and it depends on `serde`, `serde_json` and `thiserror` only, so a plugin written in Rust
//! depends on this crate and nothing of the viewer's.

mod capability;
mod error;
mod frame;
mod message;

pub use capability::Capability;
pub use error::ProtocolError;
pub use frame::{
    Frame, FrameDecoder, MAX_JSON_BYTES, MAX_PAYLOAD_BYTES, encode_frame, read_frame, write_frame,
};
pub use message::{
    DecodeRequest, Done, ErrorCode, ExportRequest, FactRow, FactsReply, Failure, Hello,
    HostMessage, ImageHeader, MicroRange, PROTOCOL_VERSION, PluginMessage, ProbeRequest, Progress,
    ThumbnailRequest,
};
