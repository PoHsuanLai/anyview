//! Where the caret goes along one line: by grapheme, by word, to the nth grapheme. Columns are
//! UTF-8 byte offsets into the line, always on a grapheme boundary.

use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// The boundary before `col`, or `None` at the start.
pub(crate) fn grapheme_before(line: &str, col: usize) -> Option<usize> {
    line.grapheme_indices(true)
        .map(|(at, _)| at)
        .take_while(|at| *at < col)
        .last()
}

/// The boundary after `col`, or `None` at the end.
pub(crate) fn grapheme_after(line: &str, col: usize) -> Option<usize> {
    line.grapheme_indices(true)
        .map(|(at, g)| at + g.len())
        .find(|end| *end > col)
}

/// The start of the word at or before `col`, skipping marks and spaces before it.
pub(crate) fn word_before(line: &str, col: usize) -> Option<usize> {
    line.split_word_bound_indices()
        .filter(|(at, segment)| *at < col && is_word(segment))
        .map(|(at, _)| at)
        .next_back()
}

/// The end of the word at or after `col`, skipping marks and spaces before it.
pub(crate) fn word_after(line: &str, col: usize) -> Option<usize> {
    line.split_word_bound_indices()
        .filter(|(at, segment)| at + segment.len() > col && is_word(segment))
        .map(|(at, segment)| at + segment.len())
        .next()
}

/// The word `col` is in or touches; the run of spaces or marks it is in otherwise.
pub(crate) fn word_at(line: &str, col: usize) -> Range<usize> {
    let mut found = col..col;
    for (at, segment) in line.split_word_bound_indices() {
        let end = at + segment.len();
        if col >= at && col < end {
            return at..end;
        }
        if col == end {
            found = at..end;
        }
    }
    found
}

/// How many graphemes come before `col`.
pub(crate) fn graphemes_before(line: &str, col: usize) -> usize {
    line.grapheme_indices(true)
        .take_while(|(at, _)| *at < col)
        .count()
}

/// The boundary after `n` graphemes, or the end of the line.
pub(crate) fn after_graphemes(line: &str, n: usize) -> usize {
    line.grapheme_indices(true)
        .nth(n)
        .map_or(line.len(), |(at, _)| at)
}

fn is_word(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}
