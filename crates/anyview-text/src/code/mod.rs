//! Syntax highlighting into token classes, by line range.
//!
//! The highlighter is `syntect` with its pure-Rust regex engine. A window of lines is highlighted
//! from the nearest saved parser state, so a window deep in a large file costs one pass over the
//! lines before it, the first time, and nothing after.

mod class;
mod html;
mod state;

#[cfg(test)]
mod tests;

pub use class::TokenClass;
pub use html::{TOKEN_CLASS_PREFIX, tokens_html};

use crate::bytes::ByteSource;
use crate::error::TextError;
use crate::lines::TextLines;
use anyview_core::{LineIndex, SyntaxName};
use state::{ClassCache, LineState, plain};
use std::ops::Range;
use syntect::parsing::SyntaxSet;

/// Lines between two saved parser states.
const CHECKPOINT_EVERY: u32 = 128;

/// Lines read and parsed at a time while catching up to a window.
const BATCH: u32 = 512;

/// The syntaxes the viewer highlights. Built once by whoever owns the highlighting and passed in:
/// the set is large and nothing here keeps a copy of it.
#[derive(Debug)]
pub struct Highlighter {
    set: SyntaxSet,
}

/// One of a [`Highlighter`]'s syntaxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxId(usize);

/// Names `SyntaxName` uses where the highlighter's own name differs, or where its nearest
/// language is the best it has.
const ALIASES: &[(&str, &str)] = &[
    ("shell", "Bourne Again Shell (bash)"),
    ("typescript", "JavaScript"),
    ("kotlin", "Java"),
    ("scss", "CSS"),
    ("markdown", "Markdown"),
    ("json", "JSON"),
];

impl Highlighter {
    /// The default syntaxes.
    pub fn new() -> Self {
        Highlighter {
            set: SyntaxSet::load_defaults_newlines(),
        }
    }

    /// The syntax that highlights `name`, or `None` for a language the set does not have (the
    /// text is then shown unstyled).
    pub fn syntax(&self, name: &SyntaxName) -> Option<SyntaxId> {
        let wanted = ALIASES
            .iter()
            .find(|(alias, _)| *alias == name.as_str())
            .map_or(name.as_str(), |(_, target)| *target);
        self.set
            .syntaxes()
            .iter()
            .position(|syntax| syntax.name.eq_ignore_ascii_case(wanted))
            .map(SyntaxId)
    }

    /// The syntax a fenced code block's language label names: a language name or a file
    /// extension, as `rust`, `Rust` or `rs`.
    pub fn syntax_by_token(&self, token: &str) -> Option<SyntaxId> {
        let found = self.set.find_syntax_by_token(token)?;
        self.set
            .syntaxes()
            .iter()
            .position(|syntax| syntax.name == found.name)
            .map(SyntaxId)
    }

    /// The text of `source` highlighted as `syntax`, for a snippet that is not a file (a fenced
    /// code block, a peek). With `None` every line is one plain span.
    pub fn snippet(&self, syntax: Option<SyntaxId>, source: &str) -> Vec<TokenLine> {
        let lines = crate::lines::split(source);
        let reference = syntax.and_then(|id| self.set.syntaxes().get(id.0));
        let mut state = reference.map(LineState::start);
        let mut classes = ClassCache::default();
        lines
            .into_iter()
            .zip(0u32..)
            .map(|(text, number)| TokenLine {
                number: LineIndex(number),
                spans: match state.as_mut() {
                    Some(state) => state.line(&text, &self.set, &mut classes),
                    None => plain(&text),
                },
            })
            .collect()
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Highlighter::new()
    }
}

/// A run of text with one class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSpan {
    /// What the text is.
    pub class: TokenClass,
    /// The text.
    pub text: String,
}

/// One highlighted line: spans that join to the line's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenLine {
    /// The line's position in the file, zero-based.
    pub number: LineIndex,
    /// The spans in order; never empty for a line with text.
    pub spans: Vec<TokenSpan>,
}

impl TokenLine {
    /// The line's text, the spans joined.
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

/// A text file prepared for highlighting by line range: the lines, the syntax, and the parser
/// states saved on the way.
pub struct CodeLines<B: ByteSource> {
    text: TextLines<B>,
    syntax: Option<SyntaxId>,
    saved: Vec<LineState>,
    classes: ClassCache,
}

impl<B: ByteSource> std::fmt::Debug for CodeLines<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeLines")
            .field("syntax", &self.syntax)
            .field("saved", &self.saved.len())
            .finish_non_exhaustive()
    }
}

impl<B: ByteSource> CodeLines<B> {
    /// `text` to be highlighted as `syntax`; with `None` every line is one plain span.
    pub fn new(highlighter: &Highlighter, text: TextLines<B>, syntax: Option<SyntaxId>) -> Self {
        let saved = syntax
            .and_then(|id| highlighter.set.syntaxes().get(id.0))
            .map(|reference| vec![LineState::start(reference)])
            .unwrap_or_default();
        CodeLines {
            text,
            syntax,
            saved,
            classes: ClassCache::default(),
        }
    }

    /// The file's lines, for the line count and plain reads.
    pub fn text(&self) -> &TextLines<B> {
        &self.text
    }

    /// The lines in `range` highlighted, cut to the lines that exist. Parser state is saved every
    /// 128 lines as the pass goes, so the next window near this one is cheap.
    pub fn highlight(
        &mut self,
        highlighter: &Highlighter,
        range: Range<LineIndex>,
    ) -> Result<Vec<TokenLine>, TextError> {
        let end = range.end.0.min(self.text.line_count().0);
        let start = range.start.0.min(end);
        if self.saved.is_empty() {
            return self.unstyled(start..end);
        }
        let anchor = (start / CHECKPOINT_EVERY).min(self.saved.len() as u32 - 1); // checked non-empty
        let mut state = self.saved[anchor as usize].clone();
        let mut line = anchor * CHECKPOINT_EVERY;
        let mut out = Vec::new();
        while line < end {
            let batch_end = end.min(line + BATCH);
            let batch = self.text.lines(LineIndex(line)..LineIndex(batch_end))?;
            if batch.is_empty() {
                break; // the file is shorter than its index said
            }
            for text in batch {
                if line.is_multiple_of(CHECKPOINT_EVERY)
                    && (line / CHECKPOINT_EVERY) as usize == self.saved.len()
                {
                    self.saved.push(state.clone());
                }
                let spans = state.line(&text, &highlighter.set, &mut self.classes);
                if line >= start {
                    out.push(TokenLine {
                        number: LineIndex(line),
                        spans,
                    });
                }
                line += 1;
            }
        }
        Ok(out)
    }

    fn unstyled(&self, range: Range<u32>) -> Result<Vec<TokenLine>, TextError> {
        let lines = self
            .text
            .lines(LineIndex(range.start)..LineIndex(range.end))?;
        Ok(lines
            .into_iter()
            .zip(range.start..)
            .map(|(text, number)| TokenLine {
                number: LineIndex(number),
                spans: plain(&text),
            })
            .collect())
    }
}
