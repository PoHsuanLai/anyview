//! The sparse line index: the byte offset of every 64th line, found by one streaming pass. Any
//! line is then at most 63 lines from a known offset, so a window is read without scanning, or
//! decoding, the file again.

use crate::bytes::read_range;
use crate::encoding::TextCodec;
use crate::error::TextError;
use anyview_core::ReadAt;
use anyview_core::work::Interrupt;
use std::ops::Range;

/// Lines between two recorded offsets.
const STRIDE: u32 = 64;

/// The most bytes of one line a window holds; the rest of a longer line is read past and dropped, so
/// a file that is one enormous line costs this much memory rather than its size. A person writes
/// lines of a few hundred bytes, and a minified bundle or an inlined data URI that passes a
/// mebibyte (the same size as the span [`DETECT_BYTES`](crate::DETECT_BYTES) reads to name the encoding) is not read
/// line by line anyway. Even, so a UTF-16 unit never splits.
pub const MAX_LINE_BYTES: usize = 1 << 20;

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
        source: &impl ReadAt,
        codec: TextCodec,
        content: u64,
        interrupt: &dyn Interrupt,
    ) -> Result<Self, TextError> {
        let end = source.len().0;
        let unit = codec.unit();
        let mut starts = Vec::new();
        let mut line: u32 = 0;
        if end > content {
            starts.push(content);
        }
        let mut at = content;
        while at < end {
            if interrupt.interrupted() {
                return Err(TextError::Interrupted);
            }
            let chunk = read_range(source, at..at + CHUNK)?;
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

    /// The lines `range`, each with its line break and at most [`MAX_LINE_BYTES`] of its text,
    /// read by scanning forward from the nearest recorded offset.
    pub(super) fn read(
        &self,
        source: &impl ReadAt,
        range: Range<u32>,
    ) -> Result<Vec<RawLine>, TextError> {
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

/// One line of a window as it lies in the file.
#[derive(Debug, Default)]
pub(super) struct RawLine {
    /// Up to [`MAX_LINE_BYTES`] of the line's text, then its line break if it has one.
    pub(super) bytes: Vec<u8>,
    /// Whether the line was longer than the cap and the rest of its text was dropped.
    pub(super) cut: bool,
}

impl RawLine {
    /// Adds `part` of the line's text, as much as the cap leaves room for.
    fn push_text(&mut self, part: &[u8]) {
        let room = MAX_LINE_BYTES - self.bytes.len();
        if part.len() > room {
            self.bytes.extend_from_slice(&part[..room]);
            self.cut = true;
        } else {
            self.bytes.extend_from_slice(part);
        }
    }
}

/// Whether the scan is still passing lines it was told to skip.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Skipping,
    Taking,
}

/// From `span.0`, skips `lines.0` lines and returns the next `lines.1`, each with its line break
/// and cut to [`MAX_LINE_BYTES`] of text. `span.1` is where the bytes end.
fn scan(
    source: &impl ReadAt,
    codec: TextCodec,
    span: (u64, u64),
    lines: (u32, u32),
) -> Result<Vec<RawLine>, TextError> {
    let (offset, end) = span;
    let (skip, wanted) = lines;
    let unit = codec.unit();
    let mut done: Vec<RawLine> = Vec::new();
    let mut current = RawLine::default();
    let mut phase = if skip == 0 {
        Phase::Taking
    } else {
        Phase::Skipping
    };
    let mut seen: u32 = 0;
    let mut at = offset;
    while at < end {
        let chunk = read_range(source, at..at + CHUNK)?;
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
                Phase::Taking => {
                    current.push_text(&chunk[from.unwrap_or(0)..i]);
                    current.bytes.extend_from_slice(&chunk[i..after]);
                    done.push(std::mem::take(&mut current));
                    if done.len() as u64 == u64::from(wanted) {
                        // a usize fits a u64
                        return Ok(done);
                    }
                    from = Some(after);
                }
                Phase::Skipping => {}
            }
        }
        if let Some(from) = from {
            current.push_text(&chunk[from..]);
        }
        at += chunk.len() as u64;
    }
    if !current.bytes.is_empty() {
        done.push(current); // the last line of a file with no final line break
    }
    Ok(done)
}
