//! Framing: a message is a header, its JSON and an optional binary payload.
//!
//! ```text
//! u32 LE json length | u32 LE payload length | JSON | payload
//! ```
//!
//! The payload carries pixels, so a 24-megapixel picture crosses the pipe as 96 MB of bytes
//! with no encoding on either side.

use crate::error::ProtocolError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{Read, Write};

/// The most JSON a message may carry: 1 MiB.
pub const MAX_JSON_BYTES: u32 = 1 << 20;

/// The most payload a message may carry: 512 MiB, 128 megapixels of RGBA8.
pub const MAX_PAYLOAD_BYTES: u32 = 512 << 20;

const HEADER: usize = 8;

/// A message and the bytes that came with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<T> {
    /// The decoded message.
    pub message: T,
    /// The payload; empty for a message that has none.
    pub payload: Vec<u8>,
}

/// `message` and `payload` as the bytes of one frame.
pub fn encode_frame<T: Serialize>(message: &T, payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let json = serde_json::to_vec(message).map_err(|error| ProtocolError::Unwritable {
        reason: error.to_string(),
    })?;
    let json_len = checked_json(json.len() as u64)?;
    let payload_len = checked_payload(payload.len() as u64)?;
    let mut bytes = Vec::with_capacity(HEADER + json.len() + payload.len());
    bytes.extend_from_slice(&json_len.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

fn checked_json(len: u64) -> Result<u32, ProtocolError> {
    u32::try_from(len)
        .ok()
        .filter(|len| *len <= MAX_JSON_BYTES)
        .ok_or(ProtocolError::JsonTooLarge {
            len,
            limit: u64::from(MAX_JSON_BYTES),
        })
}

fn checked_payload(len: u64) -> Result<u32, ProtocolError> {
    u32::try_from(len)
        .ok()
        .filter(|len| *len <= MAX_PAYLOAD_BYTES)
        .ok_or(ProtocolError::PayloadTooLarge {
            len,
            limit: u64::from(MAX_PAYLOAD_BYTES),
        })
}

/// Writes one frame and flushes: what a plugin's main loop calls.
pub fn write_frame<W: Write, T: Serialize>(
    out: &mut W,
    message: &T,
    payload: &[u8],
) -> Result<(), ProtocolError> {
    let bytes = encode_frame(message, payload)?;
    out.write_all(&bytes)
        .and_then(|()| out.flush())
        .map_err(|error| ProtocolError::Io { kind: error.kind() })
}

/// Reads one frame, blocking: what a plugin's main loop calls. `Closed` means the stream ended
/// between frames.
pub fn read_frame<R: Read, T: DeserializeOwned>(input: &mut R) -> Result<Frame<T>, ProtocolError> {
    let mut header = [0u8; HEADER];
    read_exact(input, &mut header, ProtocolError::Closed)?;
    let (json_len, payload_len) = lengths(&header)?;
    let mut json = vec![0u8; json_len];
    read_exact(input, &mut json, ProtocolError::Truncated)?;
    let mut payload = vec![0u8; payload_len];
    read_exact(input, &mut payload, ProtocolError::Truncated)?;
    Ok(Frame {
        message: parse(&json)?,
        payload,
    })
}

fn read_exact<R: Read>(
    input: &mut R,
    into: &mut [u8],
    on_empty: ProtocolError,
) -> Result<(), ProtocolError> {
    let mut filled = 0;
    while filled < into.len() {
        match input.read(&mut into[filled..]) {
            Ok(0) if filled == 0 => return Err(on_empty),
            Ok(0) => return Err(ProtocolError::Truncated),
            Ok(n) => filled += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(ProtocolError::Io { kind: error.kind() }),
        }
    }
    Ok(())
}

/// The two lengths a header announces, inside the limits.
fn lengths(header: &[u8; HEADER]) -> Result<(usize, usize), ProtocolError> {
    let word = |at: usize| {
        u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let json = checked_json(u64::from(word(0)))?;
    let payload = checked_payload(u64::from(word(4)))?;
    Ok((json as usize, payload as usize))
}

fn parse<T: DeserializeOwned>(json: &[u8]) -> Result<T, ProtocolError> {
    serde_json::from_slice(json).map_err(|error| ProtocolError::Malformed {
        reason: error.to_string(),
    })
}

/// Cuts frames out of bytes as they arrive, for a reader that must not block (the host polls the
/// pipe so it can time out). Push what was read, then pull frames until `None`.
#[derive(Debug, Clone, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    /// A decoder with nothing buffered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds bytes that arrived.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes arrive. A header over the limits or JSON
    /// that is not a `T` is an error, and the decoder is then spent.
    pub fn next_frame<T: DeserializeOwned>(&mut self) -> Result<Option<Frame<T>>, ProtocolError> {
        let Some(header) = self.buffer.first_chunk::<HEADER>() else {
            return Ok(None);
        };
        let (json_len, payload_len) = lengths(header)?;
        let total = HEADER + json_len + payload_len;
        if self.buffer.len() < total {
            return Ok(None);
        }
        let message = parse(&self.buffer[HEADER..HEADER + json_len])?;
        // Whatever follows this frame stays; the frame's own bytes become its payload.
        let rest = self.buffer.split_off(total);
        let mut payload = std::mem::replace(&mut self.buffer, rest);
        payload.drain(..HEADER + json_len);
        Ok(Some(Frame { message, payload }))
    }

    /// Whether bytes of an unfinished frame are held: a stream that ends now was cut off.
    pub fn is_mid_frame(&self) -> bool {
        !self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{HostMessage, ImageHeader, PluginMessage, ProbeRequest};

    fn probe() -> HostMessage {
        HostMessage::Probe(ProbeRequest { path: "/a".into() })
    }

    #[test]
    fn a_frame_survives_a_blocking_round_trip() {
        let image = PluginMessage::Image(ImageHeader {
            width: 2,
            height: 1,
        });
        let mut wire = Vec::new();
        write_frame(&mut wire, &image, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let frame: Frame<PluginMessage> = read_frame(&mut wire.as_slice()).unwrap();
        assert_eq!(frame.message, image);
        assert_eq!(frame.payload, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn the_decoder_waits_for_every_byte_and_cuts_back_to_back_frames() {
        let one = encode_frame(&probe(), &[]).unwrap();
        let two = encode_frame(&probe(), &[9; 5]).unwrap();
        let wire = [one.clone(), two].concat();
        let mut decoder = FrameDecoder::new();
        // One byte at a time: nothing is a frame until its last byte arrives.
        let mut got = Vec::new();
        for (at, byte) in wire.iter().enumerate() {
            decoder.push(&[*byte]);
            while let Some(frame) = decoder.next_frame::<HostMessage>().unwrap() {
                got.push((at + 1, frame.payload.len()));
            }
        }
        assert_eq!(got, [(one.len(), 0), (wire.len(), 5)]);
        assert!(!decoder.is_mid_frame());
    }

    #[test]
    fn a_header_over_the_limits_is_refused_before_buffering_its_body() {
        const CASES: &[(&str, u32, u32)] = &[
            ("json", MAX_JSON_BYTES + 1, 0),
            ("payload", 2, MAX_PAYLOAD_BYTES + 1),
        ];
        for (name, json, payload) in CASES {
            let mut header = Vec::new();
            header.extend_from_slice(&json.to_le_bytes());
            header.extend_from_slice(&payload.to_le_bytes());
            let mut decoder = FrameDecoder::new();
            decoder.push(&header);
            assert!(decoder.next_frame::<HostMessage>().is_err(), "{name}");
        }
    }

    #[test]
    fn garbage_json_and_cut_streams_are_typed_errors() {
        let mut wire = Vec::new();
        wire.extend_from_slice(&3u32.to_le_bytes());
        wire.extend_from_slice(&0u32.to_le_bytes());
        wire.extend_from_slice(b"{no");
        assert!(matches!(
            read_frame::<_, HostMessage>(&mut wire.as_slice()),
            Err(ProtocolError::Malformed { .. })
        ));
        let whole = encode_frame(&probe(), &[]).unwrap();
        assert_eq!(
            read_frame::<_, HostMessage>(&mut &whole[..whole.len() - 1]).unwrap_err(),
            ProtocolError::Truncated
        );
        assert_eq!(
            read_frame::<_, HostMessage>(&mut &[][..]).unwrap_err(),
            ProtocolError::Closed
        );
    }
}
