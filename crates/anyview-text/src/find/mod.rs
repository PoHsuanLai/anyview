//! Finding text in a file: every place a typed phrase occurs, read window by window so a large
//! file is never held whole. Matching ignores case and works on whole characters, so a hit's
//! byte range always cuts the line at character boundaries.

mod needle;

#[cfg(test)]
mod tests;

pub use needle::{ByteOffset, FindHit, Needle};

use crate::error::TextError;
use crate::lines::TextLines;
use anyview_core::work::{Stop, StopState};
use anyview_core::{LineIndex, ReadAt};

/// The most hits one search keeps: a phrase that occurs more often than this is not a search
/// the reader can step through, and the list would be most of the file.
pub const MAX_HITS: usize = 10_000;

/// How many lines one read takes.
const BATCH: u32 = 2048;

impl<B: ReadAt> TextLines<B> {
    /// Every hit of `needle`, in file order, at most [`MAX_HITS`]. Reads the whole file once, in
    /// batches of lines. Blocking; when `stop` is raised it returns the hits found so far, which
    /// the caller that raised it no longer wants.
    pub fn find(&self, needle: &Needle, stop: &Stop) -> Result<Vec<FindHit>, TextError> {
        let total = self.line_count().0;
        let mut hits = Vec::new();
        let mut first = 0;
        while first < total && hits.len() < MAX_HITS && stop.stopped() == StopState::Running {
            let end = first.saturating_add(BATCH).min(total);
            let batch = self.lines(LineIndex(first)..LineIndex(end))?;
            if batch.is_empty() {
                break; // the file is shorter than its index said
            }
            for (line, text) in (first..).zip(&batch) {
                hits.extend(needle.hits_in(LineIndex(line), text));
            }
            first = end;
        }
        hits.truncate(MAX_HITS);
        Ok(hits)
    }
}
