//! Highlighting one line at a time: the parser's state carried from line to line.

use super::TokenSpan;
use super::class::{TokenClass, class_of};
use std::collections::HashMap;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

/// Lines longer than this many bytes are not parsed: minified code makes the regex engine crawl,
/// and a window of one such line is unreadable anyway. They come back as one plain span.
const MAX_PARSED_LINE: usize = 4096;

/// The parser at a line boundary: cheap to clone, so it can be kept every so many lines.
#[derive(Clone)]
pub(super) struct LineState {
    parse: ParseState,
    stack: ScopeStack,
}

/// What each scope the parser has met maps to, remembered because building a scope's name
/// allocates.
#[derive(Default)]
pub(super) struct ClassCache(HashMap<Scope, Option<TokenClass>>);

impl ClassCache {
    /// The class of the innermost scope on `stack` that has one.
    fn of(&mut self, stack: &ScopeStack) -> TokenClass {
        for scope in stack.as_slice().iter().rev() {
            let class = *self
                .0
                .entry(*scope)
                .or_insert_with(|| class_of(&scope.build_string()));
            if let Some(class) = class {
                return class;
            }
        }
        TokenClass::Plain
    }
}

impl LineState {
    /// The state before the first line of a file in `syntax`.
    pub(super) fn start(syntax: &SyntaxReference) -> Self {
        LineState {
            parse: ParseState::new(syntax),
            stack: ScopeStack::new(),
        }
    }

    /// The spans of `text` (one line, without its line break), and the state after it. A line the
    /// parser refuses, or one that is too long, is one plain span and leaves the state as it was.
    pub(super) fn line(
        &mut self,
        text: &str,
        set: &SyntaxSet,
        classes: &mut ClassCache,
    ) -> Vec<TokenSpan> {
        if text.len() > MAX_PARSED_LINE {
            return plain(text);
        }
        // The default syntaxes are written for lines that end in a line feed.
        let with_break = format!("{text}\n");
        let before = self.clone();
        let Ok(ops) = self.parse.parse_line(&with_break, set) else {
            *self = before;
            return plain(text);
        };
        let mut spans: Vec<TokenSpan> = Vec::new();
        let mut at = 0;
        for (index, op) in ops {
            let index = index.min(text.len());
            if index > at && text.is_char_boundary(index) {
                push(&mut spans, classes.of(&self.stack), &text[at..index]);
                at = index;
            }
            if self.stack.apply(&op).is_err() {
                *self = before;
                return plain(text);
            }
        }
        if at < text.len() {
            push(&mut spans, classes.of(&self.stack), &text[at..]);
        }
        spans
    }
}

/// A line with no styling.
pub(super) fn plain(text: &str) -> Vec<TokenSpan> {
    vec![TokenSpan {
        class: TokenClass::Plain,
        text: text.to_owned(),
    }]
}

/// Appends `text`, joining it to the span before when they share a class.
fn push(spans: &mut Vec<TokenSpan>, class: TokenClass, text: &str) {
    match spans.last_mut() {
        Some(last) if last.class == class => last.text.push_str(text),
        Some(_) | None => spans.push(TokenSpan {
            class,
            text: text.to_owned(),
        }),
    }
}
