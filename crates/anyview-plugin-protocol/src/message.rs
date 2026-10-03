//! The messages of protocol version 1.
//!
//! A plugin is started once for each request. It speaks first with `Hello`; the host then sends
//! one request; the plugin answers and exits. During an export the host may send `Cancel`.

use crate::capability::Capability;
use crate::error::ProtocolError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The version of the protocol this crate speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// What the host says to a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum HostMessage {
    /// Read a file's facts.
    Probe(ProbeRequest),
    /// Make a small picture of a file.
    Thumbnail(ThumbnailRequest),
    /// Make a picture of a file within a pixel budget.
    Decode(DecodeRequest),
    /// Write a file as another format.
    Export(ExportRequest),
    /// Stop the export; the plugin removes its partial output and answers `Error` with
    /// `ErrorCode::Cancelled`.
    Cancel,
}

/// What a plugin says to the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum PluginMessage {
    /// The first message, sent before anything is asked.
    Hello(Hello),
    /// The answer to `Probe`.
    Facts(FactsReply),
    /// The answer to `Thumbnail` and `Decode`: the frame's payload is the pixels.
    Image(ImageHeader),
    /// How far an export has come; any number, in order.
    Progress(Progress),
    /// The export is finished.
    Done(Done),
    /// The request failed; the plugin then exits.
    Error(Failure),
}

/// The plugin's greeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// The protocol version the plugin speaks.
    pub protocol: u32,
    /// The plugin's name, for logs.
    pub name: String,
    /// The requests it answers.
    pub provides: Vec<Capability>,
}

/// `Probe`: facts about a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeRequest {
    /// An absolute path.
    pub path: PathBuf,
}

/// `Thumbnail`: a picture no larger than `max_edge` on its longer side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThumbnailRequest {
    /// An absolute path.
    pub path: PathBuf,
    /// The longest edge the picture may have, in pixels.
    pub max_edge: u32,
}

/// `Decode`: a picture with at most `max_area` pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodeRequest {
    /// An absolute path.
    pub path: PathBuf,
    /// Width times height may not exceed this; the plugin scales down to fit.
    pub max_area: u64,
}

/// A span of a recording, in microseconds from its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicroRange {
    /// Where it starts.
    pub start: u64,
    /// Where it ends.
    pub end: u64,
}

/// `Export`: write `input` to `output` as the manifest's target `target`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRequest {
    /// An absolute path to read.
    pub input: PathBuf,
    /// An absolute path to write. The plugin writes it only on success.
    pub output: PathBuf,
    /// One of the targets the manifest lists for `export`.
    pub target: String,
    /// The part of the recording to keep; the whole of it when absent.
    #[serde(default)]
    pub range: Option<MicroRange>,
    /// The stream to keep, by index; the default streams when absent.
    #[serde(default)]
    pub stream: Option<u32>,
}

/// One row of a probe's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactRow {
    /// The row's label, as the viewer's `FactLabel` slug (`codec`, `duration`, `title`…). A label
    /// the viewer does not know is dropped.
    pub label: String,
    /// The value, already formatted for a person.
    pub value: String,
}

/// The answer to `Probe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactsReply {
    /// The rows, in the order to show them.
    pub rows: Vec<FactRow>,
}

/// The header of an image; the frame's payload holds `width * height * 4` bytes of straight
/// (not premultiplied) RGBA8 in rows from the top, already upright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageHeader {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl ImageHeader {
    /// Checks that `payload_len` is exactly the pixels this header announces.
    pub fn check(&self, payload_len: usize) -> Result<(), ProtocolError> {
        let wanted = u64::from(self.width) * u64::from(self.height) * 4;
        let have = u64::try_from(payload_len).unwrap_or(u64::MAX);
        if wanted == have && wanted > 0 {
            Ok(())
        } else {
            Err(ProtocolError::ImageSize {
                width: self.width,
                height: self.height,
                wanted,
                have,
            })
        }
    }
}

/// How far an export has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Parts done, in any unit the plugin likes.
    pub done: u64,
    /// Parts in all; zero when the plugin cannot tell.
    pub total: u64,
}

/// An export is finished.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    /// The file written, when it differs from the one asked for.
    #[serde(default)]
    pub output: Option<PathBuf>,
}

/// Why a request failed, as a host acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The plugin does not handle this kind of file or target.
    Unsupported,
    /// The file cannot be read.
    Unreadable,
    /// The file is damaged.
    Corrupt,
    /// The file is beyond what the plugin will do.
    TooLarge,
    /// The host asked it to stop.
    Cancelled,
    /// Anything else.
    Failed,
}

/// A request that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// What a host can act on.
    pub code: ErrorCode,
    /// What a person can read.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_messages_round_trip_with_the_documented_shape() {
        // name, message, json
        let cases: Vec<(&str, HostMessage, &str)> = vec![
            (
                "probe",
                HostMessage::Probe(ProbeRequest {
                    path: "/a/b.mkv".into(),
                }),
                r#"{"kind":"probe","v":{"path":"/a/b.mkv"}}"#,
            ),
            (
                "thumbnail",
                HostMessage::Thumbnail(ThumbnailRequest {
                    path: "/a".into(),
                    max_edge: 256,
                }),
                r#"{"kind":"thumbnail","v":{"path":"/a","max_edge":256}}"#,
            ),
            (
                "decode",
                HostMessage::Decode(DecodeRequest {
                    path: "/a".into(),
                    max_area: 24_000_000,
                }),
                r#"{"kind":"decode","v":{"path":"/a","max_area":24000000}}"#,
            ),
            ("cancel", HostMessage::Cancel, r#"{"kind":"cancel"}"#),
        ];
        for (name, message, json) in cases {
            assert_eq!(serde_json::to_string(&message).unwrap(), json, "{name}");
            let back: HostMessage = serde_json::from_str(json).unwrap();
            assert_eq!(back, message, "{name}");
        }
    }

    #[test]
    fn an_export_request_without_optional_fields_still_reads() {
        let json = r#"{"kind":"export","v":{"input":"/i","output":"/o","target":"mp3"}}"#;
        let message: HostMessage = serde_json::from_str(json).unwrap();
        let HostMessage::Export(request) = message else {
            panic!("an export");
        };
        assert_eq!(request.range, None);
        assert_eq!(request.stream, None);
    }

    #[test]
    fn plugin_messages_round_trip() {
        let messages = vec![
            PluginMessage::Hello(Hello {
                protocol: PROTOCOL_VERSION,
                name: "fake".into(),
                provides: vec![Capability::Probe, Capability::Export],
            }),
            PluginMessage::Facts(FactsReply {
                rows: vec![FactRow {
                    label: "codec".into(),
                    value: "h264".into(),
                }],
            }),
            PluginMessage::Image(ImageHeader {
                width: 2,
                height: 1,
            }),
            PluginMessage::Progress(Progress { done: 1, total: 4 }),
            PluginMessage::Done(Done { output: None }),
            PluginMessage::Error(Failure {
                code: ErrorCode::Cancelled,
                message: "stopped".into(),
            }),
        ];
        for message in messages {
            let json = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<PluginMessage>(&json).unwrap(),
                message
            );
        }
    }

    #[test]
    fn an_image_header_accepts_exactly_its_pixels() {
        const CASES: &[(&str, u32, u32, usize, bool)] = &[
            ("exact", 2, 1, 8, true),
            ("short", 2, 1, 7, false),
            ("long", 2, 1, 9, false),
            ("empty", 0, 3, 0, false),
        ];
        for (name, width, height, len, ok) in CASES {
            let header = ImageHeader {
                width: *width,
                height: *height,
            };
            assert_eq!(header.check(*len).is_ok(), *ok, "{name}");
        }
    }
}
