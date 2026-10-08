//! Windowed reading of text: the line count of a file of any size, and any range of its lines,
//! decoded without decoding the rest.

mod table;

#[cfg(test)]
mod tests;

use crate::bytes::read_range;
use crate::encoding::{Coverage, DETECT_BYTES, TextCodec, detect};
use crate::error::TextError;
use anyview_core::{LineIndex, ReadAt};
use std::ops::Range;
use table::LineTable;

/// How many lines a text has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LineCount(pub u32);

/// A text file indexed for windowing: its encoding, its line count, and the means to read any run
/// of lines. Opening streams the file once; asking for lines afterwards reads only those lines
/// and, at worst, the 63 before them.
#[derive(Debug, Clone)]
pub struct TextLines<B: ReadAt> {
    bytes: B,
    codec: TextCodec,
    table: LineTable,
}

impl<B: ReadAt> TextLines<B> {
    /// Detects the encoding from the first [`DETECT_BYTES`] of `bytes` and indexes its lines.
    pub fn open(bytes: B) -> Result<Self, TextError> {
        let head = read_range(&bytes, 0..DETECT_BYTES as u64)?; // a usize fits a u64
        let coverage = if bytes.len().0 > head.len() as u64 {
            Coverage::Prefix
        } else {
            Coverage::Whole
        };
        let detected = detect(&head, coverage);
        let table = LineTable::build(&bytes, detected.codec, u64::from(detected.mark))?;
        Ok(TextLines {
            bytes,
            codec: detected.codec,
            table,
        })
    }

    /// The encoding the bytes are read as.
    pub fn encoding(&self) -> TextCodec {
        self.codec
    }

    /// The number of lines. An empty file has none, and a final line break does not start one.
    pub fn line_count(&self) -> LineCount {
        LineCount(self.table.lines())
    }

    /// The lines in `range`, without their line breaks (`\n` or `\r\n`). A range past the end is
    /// cut to the lines that exist.
    pub fn lines(&self, range: Range<LineIndex>) -> Result<Vec<String>, TextError> {
        let bytes = self.table.read(&self.bytes, range.start.0..range.end.0)?;
        Ok(split(&self.codec.decode(&bytes)))
    }
}

/// Text cut into lines at `\n`, each with a trailing `\r` removed. A final line break ends the last
/// line instead of starting an empty one.
pub(crate) fn split(text: &str) -> Vec<String> {
    split_start(text, usize::MAX).0
}

/// The first `keep` lines of `text`, cut as [`split`] cuts them, and how many lines `text` has in
/// all: a count made without holding the lines, so a file of a million of them costs `keep`.
pub(crate) fn split_start(text: &str, keep: usize) -> (Vec<String>, usize) {
    if text.is_empty() {
        return (Vec::new(), 0);
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut start = Vec::new();
    let mut total = 0_usize;
    for line in body.split('\n') {
        if start.len() < keep {
            start.push(line.strip_suffix('\r').unwrap_or(line).to_owned());
        }
        total += 1;
    }
    (start, total)
}
