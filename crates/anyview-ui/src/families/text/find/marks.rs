//! How a highlighted line is cut where a hit starts and ends, and which pieces are hits.

use anyview_text::{FindHit, TokenClass, TokenLine};

/// Whether a piece of a line is a hit, and whether it is the one the reader is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
    /// Not part of a hit.
    Plain,
    /// Part of a hit.
    Hit,
    /// Part of the current hit.
    Current,
}

impl Mark {
    /// The classes a piece of text wears for this mark, after its own.
    pub(crate) fn class(self) -> &'static str {
        match self {
            Mark::Plain => "",
            Mark::Hit => "viewer-hit",
            Mark::Current => "viewer-hit viewer-hit-current",
        }
    }
}

/// `text` cut at the edges of `hits` (byte ranges in it, in order, without overlap), each with
/// its mark and the rest plain. A range that is out of order, empty or not on character edges is
/// left out; the pieces always join back to `text`.
pub(crate) fn cut(text: &str, hits: &[(std::ops::Range<usize>, Mark)]) -> Vec<(String, Mark)> {
    let mut out = Vec::new();
    let mut at = 0;
    for (range, mark) in hits {
        let fits = at <= range.start
            && range.start < range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end);
        if !fits {
            continue;
        }
        if at < range.start {
            out.push((text[at..range.start].to_owned(), Mark::Plain));
        }
        out.push((text[range.clone()].to_owned(), *mark));
        at = range.end;
    }
    if at < text.len() || out.is_empty() {
        out.push((text[at..].to_owned(), Mark::Plain));
    }
    out
}

/// A run of one class and one mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Piece {
    pub class: TokenClass,
    pub text: String,
    pub mark: Mark,
}

/// `line` cut at the edges of `hits` (which are on it, in order, without overlap), each piece
/// keeping its token class. `current` is the position within `hits` of the hit the reader is on.
pub(crate) fn pieces(line: &TokenLine, hits: &[FindHit], current: Option<usize>) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut start = 0_usize;
    for span in &line.spans {
        let end = start + span.text.len();
        let mut cursor = start;
        for (position, hit) in hits.iter().enumerate() {
            let (from, to) = (hit.from.0 as usize, hit.to.0 as usize);
            let (from, to) = (from.max(cursor), to.min(end));
            // An edge in the middle of a character is the search and the highlighter disagreeing:
            // that hit is left unmarked rather than cut the text there.
            if from >= to
                || !span.text.is_char_boundary(from - start)
                || !span.text.is_char_boundary(to - start)
            {
                continue;
            }
            push(
                &mut out,
                span.class,
                &span.text[cursor - start..from - start],
                Mark::Plain,
            );
            let mark = if current == Some(position) {
                Mark::Current
            } else {
                Mark::Hit
            };
            push(
                &mut out,
                span.class,
                &span.text[from - start..to - start],
                mark,
            );
            cursor = to;
        }
        push(
            &mut out,
            span.class,
            &span.text[cursor - start..],
            Mark::Plain,
        );
        start = end;
    }
    out
}

fn push(out: &mut Vec<Piece>, class: TokenClass, text: &str, mark: Mark) {
    if !text.is_empty() {
        out.push(Piece {
            class,
            text: text.to_owned(),
            mark,
        });
    }
}
