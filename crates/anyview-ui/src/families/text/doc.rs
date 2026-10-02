//! A text file opened for windowing: its line index, the highlighter's saved parser states, and
//! for Markdown the rendered page. Reading a window of lines blocks, so it runs on a worker
//! (`Job::Lines`); the UI thread only ever holds the lines a worker returned.

use crate::io::{DiskFiles, OpenError, OpenLink};
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, LineIndex, Sniffed, Source,
};
use anyview_text::{
    CodeLines, FileBytes, Highlighter, LineCount, RenderEnv, Rendered, SyntaxId, TextLines,
    TokenLine, render,
};
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

/// The largest Markdown file that is rendered as a page: layout runs on the UI thread, so a larger
/// one is shown as its source.
const RENDER_LIMIT: u64 = 2 * 1024 * 1024;

/// An opened text, code, Markdown, table or JSON file.
#[derive(Debug)]
pub struct TextDoc {
    code: Mutex<CodeLines<FileBytes>>,
    highlighter: Arc<Highlighter>,
    count: LineCount,
    /// The page, for a Markdown file small enough to lay out.
    pub rendered: Option<Rendered>,
    /// The rows of the Info tab.
    pub facts: Facts,
}

/// Highlighted lines read from `first`.
#[derive(Debug, Clone)]
pub struct LineWindow {
    /// The line the first of `lines` is.
    pub first: LineIndex,
    /// The lines, highlighted, in order.
    pub lines: Vec<TokenLine>,
}

impl LineWindow {
    /// Whether the window holds every line in `range`.
    pub fn covers(&self, range: Range<u32>) -> bool {
        let end = self
            .first
            .0
            .saturating_add(u32::try_from(self.lines.len()).unwrap_or(u32::MAX));
        range.start >= self.first.0 && range.end <= end
    }
}

impl TextDoc {
    /// How many lines the file has.
    pub fn line_count(&self) -> LineCount {
        self.count
    }

    /// `rows` lines from `first`, highlighted. Blocking.
    pub fn window(
        &self,
        first: LineIndex,
        rows: u32,
    ) -> Result<LineWindow, anyview_text::TextError> {
        let end = LineIndex(first.0.saturating_add(rows));
        let lines = self
            .code
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .highlight(&self.highlighter, first..end)?;
        Ok(LineWindow { first, lines })
    }
}

/// Open the text file `src`. Blocking: indexes every line once, renders Markdown.
pub(crate) fn open(src: &Source, sniffed: &Sniffed, link: &OpenLink) -> Result<TextDoc, OpenError> {
    let text = TextLines::open(FileBytes::open(src)?)?;
    let count = text.line_count();
    let encoding = text.encoding().label();
    let rendered = markdown(src, sniffed, &text, count, &link.highlighter)?;
    let syntax = syntax_of(sniffed, &link.highlighter);
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
        .with(FactLabel::Size, FactValue::size(src.stamp().len))
        .with(FactLabel::Lines, FactValue::text(count.0.to_string()))
        .with(FactLabel::Encoding, FactValue::text(encoding));
    Ok(TextDoc {
        code: Mutex::new(CodeLines::new(&link.highlighter, text, syntax)),
        highlighter: Arc::clone(&link.highlighter),
        count,
        rendered,
        facts,
    })
}

fn syntax_of(sniffed: &Sniffed, highlighter: &Highlighter) -> Option<SyntaxId> {
    match (sniffed.kind(), sniffed.detail()) {
        (_, FormatDetail::Code(name)) => highlighter.syntax(name),
        (FormatKind::Markdown, _) => highlighter.syntax_by_token("markdown"),
        (FormatKind::Tree, _) => highlighter.syntax_by_token("json"),
        _ => None,
    }
}

fn markdown(
    src: &Source,
    sniffed: &Sniffed,
    text: &TextLines<FileBytes>,
    count: LineCount,
    highlighter: &Highlighter,
) -> Result<Option<Rendered>, OpenError> {
    if sniffed.kind() != FormatKind::Markdown || src.stamp().len.0 > RENDER_LIMIT {
        return Ok(None);
    }
    let lines = text.lines(LineIndex(0)..LineIndex(count.0))?;
    let base = src.path().parent();
    let env = RenderEnv {
        base: base.as_ref(),
        files: &DiskFiles,
        highlighter: Some(highlighter),
    };
    Ok(Some(render(&lines.join("\n"), &env)))
}
