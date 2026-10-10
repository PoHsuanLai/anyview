//! A [`Buffer`] with the caret or selection, the IME's preedit and the commands a key, a click or
//! the clipboard means. All offsets are UTF-8 bytes into the whole text.

use super::buffer::{Buffer, Kind, Revision};
use super::motion;
use super::text::{Bom, EditText};
use crate::find::{FindHit, MAX_HITS, Needle};
use anyview_core::LineIndex;
use std::ops::Range;

/// How many lines a find reads at a time.
const BATCH: usize = 2048;

/// A caret (anchor equal to focus) or a selection: it began at `anchor` and ends at `focus`,
/// either may come first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    /// Where the selection began.
    pub anchor: usize,
    /// Where it ends now: the caret.
    pub focus: usize,
}

impl Selection {
    /// A caret at `at`.
    pub fn caret(at: usize) -> Self {
        Selection {
            anchor: at,
            focus: at,
        }
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }

    /// The selected bytes, from the earlier end to the later.
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
}

/// The text an IME is composing: shown at the caret, in the buffer only once committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    /// The composing text.
    pub text: String,
    /// The IME's cursor or highlight in `text` (UTF-8 bytes).
    pub cursor: Option<Range<usize>>,
}

/// A way to move the caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// One grapheme back, over a line break to the end of the line before.
    Left,
    /// One grapheme on.
    Right,
    /// To the start of the word before.
    WordLeft,
    /// To the end of the word after.
    WordRight,
    /// To the start of the line.
    LineStart,
    /// To the end of the line.
    LineEnd,
    /// To the start of the text.
    DocStart,
    /// To the end of the text.
    DocEnd,
    /// A line up, keeping the column; the start of the text from the first line.
    Up,
    /// A line down, keeping the column; the end of the text from the last line.
    Down,
}

/// Text being edited: the buffer, the caret or selection, and the IME's preedit.
#[derive(Debug)]
pub struct Session {
    buffer: Buffer,
    bom: Bom,
    selection: Selection,
    preedit: Option<Preedit>,
    /// The column (in graphemes) vertical motion aims for, kept across a run of Up and Down.
    goal: Option<usize>,
}

impl Session {
    /// A session over `text` as it was read, the caret at the start.
    pub fn open(text: EditText) -> Self {
        let (bytes, bom) = text.into_parts();
        Session::over(Buffer::new(bytes), bom)
    }

    /// A session over `buffer`, the caret at the start.
    pub(crate) fn over(buffer: Buffer, bom: Bom) -> Self {
        Session {
            buffer,
            bom,
            selection: Selection::default(),
            preedit: None,
            goal: None,
        }
    }

    /// The text being edited.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// The text as the file holds it: its byte-order mark, if it had one, then the text.
    pub fn file_bytes(&self) -> Vec<u8> {
        let mut out = self.bom.mark().to_vec();
        out.extend(self.buffer.to_bytes());
        out
    }

    /// The stamp of the text now.
    pub fn revision(&self) -> Revision {
        self.buffer.revision()
    }

    /// Whether the text has changes that are not saved.
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// The text of `revision` is on disk.
    pub fn mark_saved(&mut self, revision: Revision) {
        self.buffer.mark_clean(revision);
    }

    /// The caret or selection.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// The caret: where the selection ends.
    pub fn caret(&self) -> usize {
        self.selection.focus
    }

    /// The composition under way, if any.
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    /// The line and the column in it (bytes) of `offset`.
    pub fn place(&self, offset: usize) -> (usize, usize) {
        let line = self.buffer.line_of(offset);
        let range = self.buffer.line_range(line);
        (line, offset.min(range.end).saturating_sub(range.start))
    }

    /// The offset one grapheme before `offset` on its line; `offset` itself at a line's start.
    pub fn grapheme_before(&self, offset: usize) -> usize {
        let (line, col) = self.place(offset);
        let before = motion::grapheme_before(&self.buffer.line_text(line), col).unwrap_or(col);
        self.buffer.line_range(line).start + before
    }

    /// The offset of column `col` in `line`, clamped to the line's text.
    pub fn offset_of(&self, line: usize, col: usize) -> usize {
        let range = self
            .buffer
            .line_range(line.min(self.buffer.line_count() - 1));
        let text = self.buffer.slice(range.clone());
        let mut col = col.min(text.len());
        while !text.is_char_boundary(col) {
            col -= 1;
        }
        range.start + col
    }

    /// Every place `needle` occurs in the text, in order, at most [`MAX_HITS`].
    pub fn find(&self, needle: &Needle) -> Vec<FindHit> {
        let total = self.buffer.line_count();
        let mut hits = Vec::new();
        let mut first = 0;
        while first < total && hits.len() < MAX_HITS {
            let batch = self.buffer.lines(first, BATCH);
            for (number, text) in (first..).zip(&batch) {
                let line = LineIndex(u32::try_from(number).unwrap_or(u32::MAX));
                hits.extend(needle.hits_in(line, text));
            }
            first += BATCH;
        }
        hits.truncate(MAX_HITS);
        hits
    }

    /// The caret at `at`, nothing selected; the run of typing under way ends.
    pub fn set_caret(&mut self, at: usize) {
        self.buffer.seal();
        self.put_caret(at);
    }

    /// The selection from `anchor` to `focus`; the run of typing under way ends.
    pub fn select(&mut self, anchor: usize, focus: usize) {
        self.buffer.seal();
        self.selection = Selection {
            anchor: self.clamp(anchor),
            focus: self.clamp(focus),
        };
        self.goal = None;
    }

    /// The selection extended (or shrunk) to end at `focus`, keeping its anchor.
    pub fn extend_to(&mut self, focus: usize) {
        self.buffer.seal();
        self.selection.focus = self.clamp(focus);
        self.goal = None;
    }

    /// Everything selected.
    pub fn select_all(&mut self) {
        self.select(0, self.buffer.len());
    }

    /// The word at `at` selected.
    pub fn select_word(&mut self, at: usize) {
        let (line, col) = self.place(at);
        let word = motion::word_at(&self.buffer.line_text(line), col);
        let start = self.buffer.line_range(line).start;
        self.select(start + word.start, start + word.end);
    }

    /// The line at `at` selected with its break.
    pub fn select_line(&mut self, at: usize) {
        let line = self.buffer.line_of(at);
        let start = self.buffer.line_start(line);
        let end = if line + 1 < self.buffer.line_count() {
            self.buffer.line_start(line + 1)
        } else {
            self.buffer.len()
        };
        self.select(start, end);
    }

    /// Move the caret by `by`; with `extend` the selection grows to it, otherwise a selection
    /// first collapses to its near edge for Left and Right, and to the caret's move for the rest.
    pub fn move_by(&mut self, by: Motion, extend: bool) {
        self.buffer.seal();
        let selection = self.selection;
        let vertical = matches!(by, Motion::Up | Motion::Down);
        let goal = if vertical {
            self.goal
                .or_else(|| Some(self.graphemes_before(selection.focus)))
        } else {
            None
        };
        let kept = match (by, extend, selection.is_empty()) {
            (Motion::Left, false, false) => selection.range().start,
            (Motion::Right, false, false) => selection.range().end,
            (Motion::Up | Motion::Down, _, _) => self.target_goal(by, selection.focus, goal),
            (_, _, _) => self.target(by, selection.focus),
        };
        self.selection = if extend {
            Selection {
                anchor: selection.anchor,
                focus: kept,
            }
        } else {
            Selection::caret(kept)
        };
        self.goal = goal;
    }

    /// Insert typed `text` over the selection. A run of letters is one undo step.
    pub fn type_text(&mut self, text: &str) {
        let text = self.in_style(text);
        self.preedit = None;
        let range = self.selection.range();
        let kind = if range.is_empty() {
            Kind::Typed
        } else {
            Kind::Whole
        };
        let caret = self.buffer.edit(range, &text, kind);
        self.put_caret(caret);
    }

    /// Enter: a line break in the file's own style.
    pub fn enter(&mut self) {
        let eol = if self.crlf() { "\r\n" } else { "\n" };
        self.put(eol);
    }

    /// Delete the selection, or the grapheme before the caret.
    pub fn backspace(&mut self) {
        let range = self.selection.range();
        if !range.is_empty() {
            return self.remove(range, Kind::Whole);
        }
        let at = range.start;
        let (line, col) = self.place(at);
        let start = self.buffer.line_range(line).start;
        let from = match motion::grapheme_before(&self.buffer.line_text(line), col) {
            Some(before) => start + before,
            None if line > 0 => self.buffer.line_range(line - 1).end,
            None => return,
        };
        self.remove(from..at, Kind::Backspace);
    }

    /// Delete the selection, or the grapheme after the caret.
    pub fn delete_forward(&mut self) {
        let range = self.selection.range();
        if !range.is_empty() {
            return self.remove(range, Kind::Whole);
        }
        let at = range.start;
        let (line, col) = self.place(at);
        let text = self.buffer.line_text(line);
        let to = match motion::grapheme_after(&text, col) {
            Some(after) => self.buffer.line_range(line).start + after,
            None if line + 1 < self.buffer.line_count() => self.buffer.line_start(line + 1),
            None => return,
        };
        self.remove(at..to, Kind::DeleteForward);
    }

    /// The selected text, or `None` with nothing selected.
    pub fn selected_text(&self) -> Option<String> {
        let range = self.selection.range();
        (!range.is_empty()).then(|| self.buffer.slice(range))
    }

    /// Take the selection out and return it, for the clipboard.
    pub fn cut(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        self.remove(self.selection.range(), Kind::Whole);
        Some(text)
    }

    /// Replace the selection with pasted `text` as one undo step.
    pub fn paste(&mut self, text: &str) {
        let text = self.in_style(text);
        self.put(&text);
    }

    /// Undo the last step.
    pub fn undo(&mut self) {
        self.preedit = None;
        if let Some(caret) = self.buffer.undo() {
            self.put_caret(caret);
        }
    }

    /// Redo the last undone step.
    pub fn redo(&mut self) {
        self.preedit = None;
        if let Some(caret) = self.buffer.redo() {
            self.put_caret(caret);
        }
    }

    /// The IME shows `text` as its preedit at the caret. The first non-empty preedit replaces
    /// the selection, as typing would.
    pub fn compose(&mut self, text: &str, cursor: Option<Range<usize>>) {
        if text.is_empty() {
            self.preedit = None;
            return;
        }
        if self.preedit.is_none() && !self.selection.is_empty() {
            self.remove(self.selection.range(), Kind::Whole);
        }
        self.preedit = Some(Preedit {
            text: text.to_owned(),
            cursor,
        });
    }

    /// The composition ended with `commit` (empty when cancelled): the preedit goes, the text
    /// is typed in its place.
    pub fn end_composition(&mut self, commit: &str) {
        self.preedit = None;
        if !commit.is_empty() {
            self.type_text(commit);
        }
    }

    /// The caret at `at` after an edit: the run of typing is not ended.
    fn put_caret(&mut self, at: usize) {
        self.selection = Selection::caret(self.clamp(at));
        self.goal = None;
    }

    fn put(&mut self, text: &str) {
        self.preedit = None;
        let caret = self.buffer.edit(self.selection.range(), text, Kind::Whole);
        self.put_caret(caret);
    }

    fn remove(&mut self, range: Range<usize>, kind: Kind) {
        self.preedit = None;
        let caret = self.buffer.edit(range, "", kind);
        self.put_caret(caret);
    }

    fn clamp(&self, at: usize) -> usize {
        let (line, col) = self.place(at.min(self.buffer.len()));
        self.offset_of(line, col)
    }

    /// `text` with its line breaks as the file writes them, so a paste into a CRLF file adds no
    /// bare LF.
    fn in_style(&self, text: &str) -> String {
        let plain = normal(text);
        if self.crlf() {
            plain.replace('\n', "\r\n")
        } else {
            plain
        }
    }

    fn crlf(&self) -> bool {
        let first = self.buffer.line_range(0).end;
        first < self.buffer.len() && self.buffer.slice(first..first + 2) == "\r\n"
    }

    fn graphemes_before(&self, at: usize) -> usize {
        let (line, col) = self.place(at);
        motion::graphemes_before(&self.buffer.line_text(line), col)
    }

    fn target(&self, by: Motion, from: usize) -> usize {
        let (line, col) = self.place(from);
        let range = self.buffer.line_range(line);
        let text = self.buffer.line_text(line);
        match by {
            Motion::Left => match motion::grapheme_before(&text, col) {
                Some(before) => range.start + before,
                None if line > 0 => self.buffer.line_range(line - 1).end,
                None => from,
            },
            Motion::Right => match motion::grapheme_after(&text, col) {
                Some(after) => range.start + after,
                None if line + 1 < self.buffer.line_count() => self.buffer.line_start(line + 1),
                None => from,
            },
            Motion::WordLeft => match motion::word_before(&text, col) {
                Some(before) => range.start + before,
                None if line > 0 => self.buffer.line_range(line - 1).end,
                None => range.start,
            },
            Motion::WordRight => match motion::word_after(&text, col) {
                Some(after) => range.start + after,
                None if line + 1 < self.buffer.line_count() => self.buffer.line_start(line + 1),
                None => range.end,
            },
            Motion::LineStart => range.start,
            Motion::LineEnd => range.end,
            Motion::DocStart => 0,
            Motion::DocEnd => self.buffer.len(),
            Motion::Up | Motion::Down => from,
        }
    }

    fn target_goal(&self, by: Motion, from: usize, goal: Option<usize>) -> usize {
        let line = self.buffer.line_of(from);
        let next = match by {
            Motion::Up if line == 0 => return 0,
            Motion::Up => line - 1,
            Motion::Down if line + 1 >= self.buffer.line_count() => return self.buffer.len(),
            Motion::Down => line + 1,
            Motion::Left
            | Motion::Right
            | Motion::WordLeft
            | Motion::WordRight
            | Motion::LineStart
            | Motion::LineEnd
            | Motion::DocStart
            | Motion::DocEnd => return self.target(by, from),
        };
        let text = self.buffer.line_text(next);
        self.buffer.line_range(next).start + motion::after_graphemes(&text, goal.unwrap_or(0))
    }
}

/// Typed or pasted text with its line breaks as `\n`.
fn normal(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
