//! The sparse line index: the byte offset of every 64th line, found by one streaming pass. Any
//! line is then at most 63 lines from a known offset, so a window is read without scanning, or
//! decoding, the file again.

use crate::bytes::ByteSource;
use crate::encoding::TextCodec;
use crate::error::TextError;
use std::ops::Range;

/// Lines between two recorded offsets.
const STRIDE: u32 = 64;

/// Bytes read at a time while scanning. A multiple of 2 so a UTF-16 unit never splits.
const CHUNK: u64 = 64 * 1024;

/// Where lines start in one file's bytes, past its byte-order mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LineTable {
    /// The offset of line 0, 64, 128, ….
    starts: Vec<u64>,
    lines: u32,
    codec: TextCodec,
    end: u64,
}

impl LineTable {
    /// Streams `source` once from `content` (the first byte after any byte-order mark).
    pub(super) fn build(
        source: &impl ByteSource,
        codec: TextCodec,
        content: u64,
    ) -> Result<Self, TextError> {
        let end = source.byte_len().0;
        let unit = codec.unit();
        let mut starts = Vec::new();
        let mut line: u32 = 0;
        if end > content {
            starts.push(content);
        }
        let mut at = content;
        while at < end {
            let chunk = source.read(at..at + CHUNK)?;
            if chunk.is_empty() {
                break;
            }
            for i in (0..chunk.len()).step_by(unit) {
                let next = at + (i + unit) as u64; // a usize fits a u64
                if codec.line_feed_at(&chunk, i) && next < end {
                    line = line.saturating_add(1);
                    if line.is_multiple_of(STRIDE) {
                        starts.push(next);
                    }
                }
            }
            at += chunk.len() as u64;
        }
        let lines = if end > content {
            line.saturating_add(1)
        } else {
            0
        };
        Ok(LineTable {
            starts,
            lines,
            codec,
            end,
        })
    }

    /// How many lines the file has: a final line break does not start another.
    pub(super) fn lines(&self) -> u32 {
        self.lines
    }

    /// The bytes of lines `range`, each with its line break, read by scanning forward from the
    /// nearest recorded offset.
    pub(super) fn read(
        &self,
        source: &impl ByteSource,
        range: Range<u32>,
    ) -> Result<Vec<u8>, TextError> {
        let first = range.start.min(self.lines);
        let last = range.end.min(self.lines);
        if first >= last {
            return Ok(Vec::new());
        }
        let anchor = first / STRIDE;
        let Some(&offset) = self.starts.get(anchor as usize) else {
            return Ok(Vec::new()); // every line below `self.lines` has an anchor
        };
        let skip = first - anchor * STRIDE;
        scan(source, self.codec, (offset, self.end), (skip, last - first))
    }
}

/// Whether the scan is still passing lines it was told to skip.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Skipping,
    Taking,
}

/// From `span.0`, skips `lines.0` lines and returns the bytes of the next `lines.1`, line breaks
/// included. `span.1` is where the bytes end.
fn scan(
    source: &impl ByteSource,
    codec: TextCodec,
    span: (u64, u64),
    lines: (u32, u32),
) -> Result<Vec<u8>, TextError> {
    let (offset, end) = span;
    let (skip, wanted) = lines;
    let unit = codec.unit();
    let mut out = Vec::new();
    let mut phase = if skip == 0 {
        Phase::Taking
    } else {
        Phase::Skipping
    };
    let mut seen: u32 = 0;
    let mut at = offset;
    while at < end {
        let chunk = source.read(at..at + CHUNK)?;
        if chunk.is_empty() {
            break;
        }
        let mut from = match phase {
            Phase::Taking => Some(0),
            Phase::Skipping => None,
        };
        for i in (0..chunk.len()).step_by(unit) {
            if !codec.line_feed_at(&chunk, i) {
                continue;
            }
            seen += 1;
            let after = (i + unit).min(chunk.len());
            match phase {
                Phase::Skipping if seen == skip => {
                    phase = Phase::Taking;
                    from = Some(after);
                }
                Phase::Taking if seen == skip + wanted => {
                    out.extend_from_slice(&chunk[from.unwrap_or(0)..after]);
                    return Ok(out);
                }
                Phase::Skipping | Phase::Taking => {}
            }
        }
        if let Some(from) = from {
            out.extend_from_slice(&chunk[from..]);
        }
        at += chunk.len() as u64;
    }
    Ok(out)
}
