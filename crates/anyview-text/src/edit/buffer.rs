//! The text being edited: a piece table over the file's own bytes, with undo and redo.
//!
//! The text is a list of runs ("pieces") of the original bytes and of an append-only buffer of
//! typed bytes, so an edit moves no text. Every piece knows how many line breaks it holds, so a
//! line is found by walking the pieces and reading only the one that holds its break.
//!
//! # Undo steps
//!
//! An edit is one step to undo, except that a run of the same kind joins into one step, as it
//! does in the editors people know:
//!
//! - a run of typed characters at the caret is one step, cut where the kind of character changes
//!   (a word, its spaces, the marks after it) and at a line break;
//! - a run of Backspaces, or of forward Deletes, is one step, cut the same way;
//! - a paste, a cut, a replaced selection and a line break are each a step of their own.
//!
//! Moving the caret ends a run ([`Buffer::seal`]), so typing at one place, going elsewhere and
//! coming back is two steps. Saving ends it too, so the saved text can always be got back to.

use std::ops::Range;

/// Where a run of text lies: in the file's own bytes, or in what was typed since.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Original,
    Added,
}

#[derive(Debug, Clone, Copy)]
struct Piece {
    origin: Origin,
    start: usize,
    len: usize,
    /// How many `\n` the piece holds.
    breaks: usize,
}

/// What an edit is, which decides whether it joins the step before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A typed character at the caret.
    Typed,
    /// Backspace: the text before the caret goes.
    Backspace,
    /// Delete: the text after the caret goes.
    DeleteForward,
    /// Anything else (a paste, a cut, a replaced selection, a line break): a step of its own.
    Whole,
}

/// A stamp of the text: two stamps are equal exactly when no edit lies between them (an undo
/// that comes back to a text has its stamp again). The saved text has one, so "has unsaved
/// changes" is a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Revision(u64);

/// One step of the history: `removed` at `at` was replaced by `added`. Undo applies it backwards.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    at: usize,
    removed: String,
    added: String,
    kind: Kind,
    before: Revision,
    after: Revision,
}

/// Text being edited: its original bytes, what was typed over them, and the history of edits.
/// Offsets are UTF-8 bytes into the whole text; lines are cut at `\n`, and a final `\n` starts an
/// empty last line, as an editor shows it.
#[derive(Debug)]
pub struct Buffer {
    original: Vec<u8>,
    added: Vec<u8>,
    pieces: Vec<Piece>,
    len: usize,
    undo: Vec<Step>,
    redo: Vec<Step>,
    now: Revision,
    clean: Revision,
    next: u64,
    /// Whether the last step may still take the next edit of its kind.
    open: bool,
}

impl Buffer {
    /// A buffer of `original`, which must be valid UTF-8 (the text is only ever cut at
    /// character boundaries, and read back as text).
    pub(crate) fn new(original: Vec<u8>) -> Self {
        let len = original.len();
        let breaks = count_breaks(&original);
        let pieces = if len == 0 {
            Vec::new()
        } else {
            vec![Piece {
                origin: Origin::Original,
                start: 0,
                len,
                breaks,
            }]
        };
        Buffer {
            original,
            added: Vec::new(),
            pieces,
            len,
            undo: Vec::new(),
            redo: Vec::new(),
            now: Revision(0),
            clean: Revision(0),
            next: 1,
            open: false,
        }
    }

    /// How many bytes the text has now.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there is no text.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The stamp of the text as it is now.
    pub fn revision(&self) -> Revision {
        self.now
    }

    /// Whether the text differs from the text last marked clean (at first, the original).
    pub(crate) fn is_modified(&self) -> bool {
        self.now != self.clean
    }

    /// The text of `revision` is the saved one. The next edit starts a step of its own, so undo
    /// can come back to it.
    pub(crate) fn mark_clean(&mut self, revision: Revision) {
        self.clean = revision;
        self.open = false;
    }

    /// End the run of typing or deleting under way: the next edit is a step of its own.
    pub(crate) fn seal(&mut self) {
        self.open = false;
    }

    /// The text in `range` (cut to the text, and to character boundaries).
    pub fn slice(&self, range: Range<usize>) -> String {
        let end = self.floor(range.end.min(self.len));
        let start = self.floor(range.start.min(end));
        String::from_utf8_lossy(&self.bytes(start..end)).into_owned()
    }

    /// All of the text.
    pub fn text(&self) -> String {
        self.slice(0..self.len)
    }

    /// All of the text as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes(0..self.len)
    }

    /// How many lines there are: at least one.
    pub fn line_count(&self) -> usize {
        self.pieces.iter().map(|p| p.breaks).sum::<usize>() + 1
    }

    /// The offset `line` starts at; the end of the text for a line past the last.
    pub fn line_start(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        let mut need = line;
        let mut offset = 0;
        for piece in &self.pieces {
            if piece.breaks < need {
                need -= piece.breaks;
                offset += piece.len;
                continue;
            }
            let mut seen = 0;
            for (i, b) in self.piece_bytes(piece, 0..piece.len).iter().enumerate() {
                if *b == b'\n' {
                    seen += 1;
                    if seen == need {
                        return offset + i + 1;
                    }
                }
            }
        }
        self.len
    }

    /// Where `line`'s text lies, without its break (`\n` or `\r\n`).
    pub fn line_range(&self, line: usize) -> Range<usize> {
        let start = self.line_start(line);
        let mut end = if line + 1 < self.line_count() {
            self.line_start(line + 1) - 1
        } else {
            self.len
        };
        if end > start && self.byte_at(end - 1) == Some(b'\r') {
            end -= 1;
        }
        start..end
    }

    /// `line`'s text without its break.
    pub fn line_text(&self, line: usize) -> String {
        self.slice(self.line_range(line))
    }

    /// `count` lines from `first`, each without its break, cut to the lines there are. One walk
    /// over the pieces, however many lines.
    pub fn lines(&self, first: usize, count: usize) -> Vec<String> {
        if count == 0 {
            return Vec::new();
        }
        let total = self.line_count();
        let first = first.min(total - 1);
        let end = first.saturating_add(count).min(total);
        let from = self.line_start(first);
        let to = if end < total {
            self.line_start(end)
        } else {
            self.len
        };
        let text = self.slice(from..to);
        let body = if end < total {
            text.strip_suffix('\n').unwrap_or(&text)
        } else {
            &text
        };
        body.split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
            .collect()
    }

    /// The line `offset` is on.
    pub fn line_of(&self, offset: usize) -> usize {
        let offset = offset.min(self.len);
        let mut before = 0;
        let mut at = 0;
        for piece in &self.pieces {
            if at + piece.len <= offset {
                before += piece.breaks;
                at += piece.len;
                continue;
            }
            let within = self.piece_bytes(piece, 0..offset - at);
            return before + within.iter().filter(|b| **b == b'\n').count();
        }
        before
    }

    /// Replace `range` with `text` as the edit `kind`, joining the step before it when the two
    /// are one run. Returns where the caret goes: the end of the new text.
    pub(crate) fn edit(&mut self, range: Range<usize>, text: &str, kind: Kind) -> usize {
        let end = self.floor(range.end.min(self.len));
        let start = self.floor(range.start.min(end));
        let removed = self.slice(start..end);
        if removed.is_empty() && text.is_empty() {
            return start;
        }
        let change = Step {
            at: start,
            removed,
            added: text.to_owned(),
            kind,
            before: self.now,
            after: Revision(self.next),
        };
        self.apply(change.at, &change.removed, &change.added);
        self.redo.clear();
        let after = change.after;
        self.next += 1;
        self.now = after;
        let caret = start + text.len();
        match self.joined(&change) {
            Some(index) => {
                let last = &mut self.undo[index];
                *last = merge(last, &change);
            }
            None => self.undo.push(change),
        }
        self.open = kind != Kind::Whole;
        caret
    }

    /// Undo the last step; where the caret goes, or `None` with nothing to undo.
    pub(crate) fn undo(&mut self) -> Option<usize> {
        let step = self.undo.pop()?;
        self.apply(step.at, &step.added, &step.removed);
        self.now = step.before;
        self.open = false;
        let caret = step.at + step.removed.len();
        self.redo.push(step);
        Some(caret)
    }

    /// Redo the last undone step; where the caret goes, or `None` with nothing to redo.
    pub(crate) fn redo(&mut self) -> Option<usize> {
        let step = self.redo.pop()?;
        self.apply(step.at, &step.removed, &step.added);
        self.now = step.after;
        self.open = false;
        let caret = step.at + step.added.len();
        self.undo.push(step);
        Some(caret)
    }

    /// The index of the step `change` continues, if it continues one.
    fn joined(&self, change: &Step) -> Option<usize> {
        if !self.open || change.kind == Kind::Whole {
            return None;
        }
        let index = self.undo.len().checked_sub(1)?;
        let last = self.undo.get(index)?;
        let continues = match change.kind {
            Kind::Typed => {
                last.kind == Kind::Typed
                    && change.removed.is_empty()
                    && last.removed.is_empty()
                    && last.at + last.added.len() == change.at
                    && same_run(last.added.chars().next_back(), change.added.chars().next())
            }
            Kind::Backspace => {
                last.kind == Kind::Backspace
                    && last.added.is_empty()
                    && change.at + change.removed.len() == last.at
                    && same_run(
                        change.removed.chars().next_back(),
                        last.removed.chars().next(),
                    )
            }
            Kind::DeleteForward => {
                last.kind == Kind::DeleteForward
                    && last.added.is_empty()
                    && change.at == last.at
                    && same_run(
                        last.removed.chars().next_back(),
                        change.removed.chars().next(),
                    )
            }
            Kind::Whole => false,
        };
        continues.then_some(index)
    }

    /// Make the pieces say `added` where `removed` was, at `at`.
    fn apply(&mut self, at: usize, removed: &str, added: &str) {
        let first = self.cut_at(at);
        let last = self.cut_at(at + removed.len());
        self.pieces.drain(first..last);
        if !added.is_empty() {
            let start = self.added.len();
            self.added.extend_from_slice(added.as_bytes());
            let breaks = added.bytes().filter(|b| *b == b'\n').count();
            self.pieces.insert(
                first,
                Piece {
                    origin: Origin::Added,
                    start,
                    len: added.len(),
                    breaks,
                },
            );
        }
        self.len = self.len + added.len() - removed.len();
    }

    /// The index of the piece that starts at `offset`, splitting the piece that holds it.
    fn cut_at(&mut self, offset: usize) -> usize {
        let mut at = 0;
        for index in 0..self.pieces.len() {
            let piece = self.pieces[index];
            if offset == at {
                return index;
            }
            if offset < at + piece.len {
                let within = offset - at;
                let left_breaks = count_breaks(self.piece_bytes(&piece, 0..within));
                let left = Piece {
                    len: within,
                    breaks: left_breaks,
                    ..piece
                };
                let right = Piece {
                    start: piece.start + within,
                    len: piece.len - within,
                    breaks: piece.breaks - left_breaks,
                    ..piece
                };
                self.pieces[index] = left;
                self.pieces.insert(index + 1, right);
                return index + 1;
            }
            at += piece.len;
        }
        self.pieces.len()
    }

    /// `offset`, or the character boundary before it.
    fn floor(&self, offset: usize) -> usize {
        let mut at = offset.min(self.len);
        while at > 0 && at < self.len {
            match self.byte_at(at) {
                Some(b) if b & 0b1100_0000 == 0b1000_0000 => at -= 1,
                Some(_) | None => break,
            }
        }
        at
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        let mut at = 0;
        for piece in &self.pieces {
            if offset < at + piece.len {
                return self
                    .piece_bytes(piece, offset - at..offset - at + 1)
                    .first()
                    .copied();
            }
            at += piece.len;
        }
        None
    }

    fn bytes(&self, range: Range<usize>) -> Vec<u8> {
        let mut out = Vec::with_capacity(range.len());
        let mut at = 0;
        for piece in &self.pieces {
            let end = at + piece.len;
            if end > range.start && at < range.end {
                let from = range.start.saturating_sub(at);
                let to = range.end.min(end) - at;
                out.extend_from_slice(self.piece_bytes(piece, from..to));
            }
            at = end;
            if at >= range.end {
                break;
            }
        }
        out
    }

    fn piece_bytes(&self, piece: &Piece, within: Range<usize>) -> &[u8] {
        let (from, to) = (piece.start + within.start, piece.start + within.end);
        let source = match piece.origin {
            Origin::Added => &self.added,
            Origin::Original => &self.original,
        };
        source.get(from..to).unwrap_or_default()
    }
}

/// `last` and `change`, the second continuing the first, as one step.
fn merge(last: &Step, change: &Step) -> Step {
    match change.kind {
        Kind::Typed => Step {
            added: format!("{}{}", last.added, change.added),
            after: change.after,
            ..last.clone()
        },
        Kind::Backspace => Step {
            at: change.at,
            removed: format!("{}{}", change.removed, last.removed),
            after: change.after,
            ..last.clone()
        },
        Kind::DeleteForward => Step {
            removed: format!("{}{}", last.removed, change.removed),
            after: change.after,
            ..last.clone()
        },
        Kind::Whole => change.clone(),
    }
}

fn count_breaks(bytes: &[u8]) -> usize {
    bytes.iter().filter(|b| **b == b'\n').count()
}

/// Whether `next` continues the run that ended in `last`: a character of the same kind (a word's
/// letters, spaces, or other marks), and never a line break.
fn same_run(last: Option<char>, next: Option<char>) -> bool {
    let (Some(last), Some(next)) = (last, next) else {
        return false;
    };
    let kind = |c: char| (c.is_alphanumeric(), c.is_whitespace());
    !matches!(last, '\n' | '\r') && !matches!(next, '\n' | '\r') && kind(last) == kind(next)
}
