//! A text file opened for windowing: its line index, the highlighter's saved parser states, and
//! for Markdown the rendered page. Reading a window of lines blocks, so it runs on a worker
//! (`Job::Lines`); the UI thread only ever holds the lines a worker returned.

use super::find::{FoundHits, Snippet, clip};
use crate::io::{OpenError, OpenPort, Stop};
use crate::{Editable, TypedText};
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FilePath, FormatDetail, FormatKind, LineIndex, Sniffed,
    Source,
};
use anyview_text::{
    CodeLines, DiskFiles, EDIT_BYTES, EditRefusal, EditText, FileBytes, Highlighter, LineCount,
    Needle, RenderEnv, Rendered, Session, SyntaxId, TextLines, TokenLine, render,
};
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

/// The largest Markdown file that is rendered as a page: layout runs on the UI thread, so a larger
/// one is shown as its source.
const RENDER_LIMIT: u64 = 2 * 1024 * 1024;

/// How many hits of a find keep the words around them, for the palette's list.
const SNIPPETS: usize = 200;

/// The hits of `needle` in the text being edited, with the words around the first of them.
pub(crate) fn found_in(text: &Session, needle: &Needle) -> FoundHits {
    let hits = text.find(needle);
    let snippets = hits
        .iter()
        .take(SNIPPETS)
        .map(|hit| {
            let line = text.buffer().line_text(hit.line.0 as usize);
            clip(&line, hit.from.0 as usize, hit.to.0 as usize)
        })
        .collect();
    FoundHits::new(hits).with_snippets(snippets)
}

/// How much of a large file the first frame reads. A file no longer than this has no first frame:
/// opening it takes no longer.
const FIRST_FRAME_BYTES: u64 = 256 * 1024;

/// An opened text, code, Markdown, table or JSON file.
#[derive(Debug)]
pub struct TextDoc {
    code: Mutex<CodeLines<FileBytes>>,
    /// The same lines, for a search that must not hold up a window of lines being read.
    text: TextLines<FileBytes>,
    highlighter: Arc<Highlighter>,
    count: LineCount,
    /// The page, for a Markdown file small enough to lay out.
    pub rendered: Option<Rendered>,
    /// The rows of the Info tab.
    pub facts: Facts,
    /// The file, which editing reads whole.
    path: FilePath,
    /// How many bytes the file had when it was opened.
    len: u64,
    /// How the file is highlighted, for the text being edited.
    syntax: Option<SyntaxId>,
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
    /// The lines of the window from `line` on: none when `line` is not in it.
    pub(super) fn from(&self, line: LineIndex) -> &[TokenLine] {
        let skip = line.0.saturating_sub(self.first.0) as usize;
        if line.0 < self.first.0 {
            &[]
        } else {
            self.lines.get(skip..).unwrap_or_default()
        }
    }

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

    /// Whether the file is small enough to be edited in place.
    pub(crate) fn editable(&self) -> Editable {
        if self.len <= EDIT_BYTES {
            Editable::Yes
        } else {
            Editable::No
        }
    }

    /// The file read whole for editing, or why it is not edited. Blocking.
    pub(crate) fn read_for_edit(&self) -> Result<EditText, EditRefusal> {
        EditText::read(self.path.as_path())
    }

    /// `source` highlighted as this file is, one line of tokens for each of its lines.
    pub(crate) fn highlight(&self, source: &str) -> Vec<TokenLine> {
        self.highlighter.snippet(self.syntax, source)
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

    /// The words around the first `SNIPPETS` of `hits`, for the palette's list. A line that cannot
    /// be read leaves its hit with a clip of nothing; the hit is still listed by its line.
    fn snippets(&self, hits: &[anyview_text::FindHit]) -> Vec<Snippet> {
        let mut lines: Option<(LineIndex, String)> = None;
        hits.iter()
            .take(SNIPPETS)
            .map(|hit| {
                let text = match &lines {
                    Some((line, text)) if *line == hit.line => text.clone(),
                    Some(_) | None => {
                        let read = self
                            .text
                            .lines(hit.line..LineIndex(hit.line.0.saturating_add(1)))
                            .ok()
                            .and_then(|read| read.into_iter().next())
                            .unwrap_or_default();
                        lines = Some((hit.line, read.clone()));
                        read
                    }
                };
                clip(&text, hit.from.0 as usize, hit.to.0 as usize)
            })
            .collect()
    }

    /// Every place `query` occurs, ignoring case; none for an empty query. Blocking; the hits
    /// found so far when `stop` is raised.
    pub fn find(
        &self,
        query: &TypedText,
        stop: &Stop,
    ) -> Result<FoundHits, anyview_text::TextError> {
        match Needle::new(query.as_str()) {
            Some(needle) => {
                let hits = self.text.find(&needle, stop)?;
                let snippets = self.snippets(&hits);
                Ok(FoundHits::new(hits).with_snippets(snippets))
            }
            None => Ok(FoundHits::default()),
        }
    }
}

/// Open the text file `src`. Blocking: indexes every line once, renders Markdown.
pub(crate) fn open(src: &Source, sniffed: &Sniffed, link: &OpenPort) -> Result<TextDoc, OpenError> {
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
        text: text.clone(),
        code: Mutex::new(CodeLines::new(&link.highlighter, text, syntax)),
        highlighter: Arc::clone(&link.highlighter),
        count,
        rendered,
        facts,
        path: src.path().clone(),
        len: src.stamp().len.0,
        syntax,
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

/// The start of the text file `src`, as a document of the lines in its first bytes, or `None` when
/// the file is short enough that opening it is as quick. The facts are the file's, the line count
/// is the start's, and a Markdown file is shown as its source until it is open.
pub(crate) fn first_frame(
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenPort,
) -> Result<Option<TextDoc>, OpenError> {
    if src.stamp().len.0 <= FIRST_FRAME_BYTES {
        return Ok(None);
    }
    let text = TextLines::open(FileBytes::first(src, ByteLen(FIRST_FRAME_BYTES))?)?;
    let count = text.line_count();
    let syntax = syntax_of(sniffed, &link.highlighter);
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
        .with(FactLabel::Size, FactValue::size(src.stamp().len));
    Ok(Some(TextDoc {
        text: text.clone(),
        code: Mutex::new(CodeLines::new(&link.highlighter, text, syntax)),
        highlighter: Arc::clone(&link.highlighter),
        count,
        rendered: None,
        facts,
        path: src.path().clone(),
        len: src.stamp().len.0,
        syntax,
    }))
}
