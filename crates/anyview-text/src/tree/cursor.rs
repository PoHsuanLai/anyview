//! A position that moves over bytes read in chunks, so a document is scanned without being held.

use crate::bytes::read_range;
use crate::error::TextError;
use anyview_core::ReadAt;

/// Bytes read at a time.
const CHUNK: u64 = 64 * 1024;

/// A position in a [`ReadAt`], and the chunk of bytes around it. A read that fails ends the
/// bytes there; the failure waits in [`Cursor::failure`] for the caller that started the scan.
pub(super) struct Cursor<'a> {
    source: &'a dyn ReadAt,
    chunk: Vec<u8>,
    base: u64,
    at: u64,
    end: u64,
    failure: Option<TextError>,
}

impl<'a> Cursor<'a> {
    /// A cursor at `at` that sees the bytes of `source` before `end`.
    pub(super) fn new(source: &'a dyn ReadAt, at: u64, end: u64) -> Self {
        Cursor {
            source,
            chunk: Vec::new(),
            base: 0,
            at,
            end: end.min(source.len().0),
            failure: None,
        }
    }

    /// Where the cursor is.
    pub(super) fn pos(&self) -> u64 {
        self.at
    }

    /// Moves the cursor to `at`.
    pub(super) fn seek(&mut self, at: u64) {
        self.at = at;
    }

    /// The read failure that cut the bytes short, once.
    pub(super) fn failure(&mut self) -> Option<TextError> {
        self.failure.take()
    }

    /// The bytes from the cursor to the end of the chunk it is in: empty at the end.
    pub(super) fn rest(&mut self) -> &[u8] {
        if self.at >= self.end {
            return &[];
        }
        let held = self.at >= self.base && self.at - self.base < self.chunk.len() as u64; // a usize fits a u64
        if !held {
            let to = (self.at + CHUNK).min(self.end);
            match read_range(self.source, self.at..to) {
                Ok(bytes) => {
                    self.chunk = bytes;
                    self.base = self.at;
                }
                Err(error) => {
                    self.failure.get_or_insert(error);
                    self.end = self.at;
                    return &[];
                }
            }
            if self.chunk.is_empty() {
                self.end = self.at; // the source ended before it said it would
                return &[];
            }
        }
        let from = usize::try_from(self.at - self.base).unwrap_or(usize::MAX);
        self.chunk.get(from..).unwrap_or_default()
    }

    /// The byte under the cursor, or `None` at the end.
    pub(super) fn peek(&mut self) -> Option<u8> {
        self.rest().first().copied()
    }

    /// Moves past one byte.
    pub(super) fn bump(&mut self) {
        self.at += 1;
    }

    /// Moves past `count` bytes.
    pub(super) fn advance(&mut self, count: usize) {
        self.at += count as u64; // a usize fits a u64
    }

    /// Moves past spaces, tabs and line breaks; says how many.
    pub(super) fn skip_ws(&mut self) -> usize {
        let mut skipped = 0;
        loop {
            let rest = self.rest();
            let n = rest
                .iter()
                .take_while(|b| matches!(**b, b' ' | b'\t' | b'\n' | b'\r'))
                .count();
            let more = n > 0 && n == rest.len();
            self.advance(n);
            skipped += n;
            if !more {
                return skipped;
            }
        }
    }
}
