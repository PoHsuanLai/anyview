//! A find's hits as the text view draws them: the list a worker made, which of them are on a
//! line, how a highlighted line is cut where a hit starts and ends, and where the view scrolls
//! to show a hit. The stage machine holds only the count and the current index (`FindHits`);
//! this is the edge's side, the places themselves.

use crate::{HitCount, HitIndex};
use anyview_core::LineIndex;
use anyview_text::{FindHit, TokenClass, TokenLine};

/// The words around a hit, for the palette's list: a clip of its line and where in the clip the
/// match is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    /// The clip.
    pub text: String,
    /// The match inside it, as byte offsets.
    pub matched: std::ops::Range<usize>,
}

/// Every hit of a phrase in a file, in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FoundHits {
    hits: Vec<FindHit>,
    /// The snippets of the first hits, in the same order; the palette lists no more than these.
    snippets: Vec<Snippet>,
}

/// How much of a line is kept before a match and after it, in bytes.
const BEFORE: usize = 32;
const AFTER: usize = 72;

/// `line` clipped to the words around the bytes `from..to`: leading space gone, an ellipsis where
/// the line was cut, the match located in the result. Offsets that are not on character edges
/// (the highlighter and the search disagreeing) give the start of the line with no match marked.
pub(crate) fn clip(line: &str, from: usize, to: usize) -> Snippet {
    let from_ok = from <= to && to <= line.len();
    if !from_ok || !line.is_char_boundary(from) || !line.is_char_boundary(to) {
        let text: String = line.trim().chars().take(BEFORE + AFTER).collect();
        return Snippet {
            text,
            matched: 0..0,
        };
    }
    let mut start = from.saturating_sub(BEFORE);
    while !line.is_char_boundary(start) {
        start -= 1;
    }
    let lead = line[start..from].len() - line[start..from].trim_start().len();
    start += lead;
    let mut end = to.saturating_add(AFTER).min(line.len());
    while !line.is_char_boundary(end) {
        end += 1;
    }
    let body = line[start..end].trim_end();
    let end = (start + body.len()).max(to);
    let body = &line[start..end];
    let cut_before = start > line.len() - line.trim_start().len();
    let cut_after = end < line.trim_end().len();
    let (head, tail) = (
        if cut_before { "\u{2026}" } else { "" },
        if cut_after { "\u{2026}" } else { "" },
    );
    Snippet {
        text: format!("{head}{body}{tail}"),
        matched: head.len() + (from - start)..head.len() + (to - start),
    }
}

impl FoundHits {
    /// The hits `hits`, which a search made in file order.
    pub fn new(hits: Vec<FindHit>) -> FoundHits {
        FoundHits {
            hits,
            snippets: Vec::new(),
        }
    }

    /// The same hits with the words around the first of them.
    pub(crate) fn with_snippets(self, snippets: Vec<Snippet>) -> FoundHits {
        FoundHits { snippets, ..self }
    }

    /// The words around the hit numbered `index`, when they were kept.
    pub(crate) fn snippet(&self, index: HitIndex) -> Option<&Snippet> {
        self.snippets.get(index.0 as usize)
    }

    /// How many there are.
    pub fn count(&self) -> HitCount {
        HitCount(u32::try_from(self.hits.len()).unwrap_or(u32::MAX))
    }

    /// The hit numbered `index`.
    pub fn get(&self, index: HitIndex) -> Option<&FindHit> {
        self.hits.get(index.0 as usize)
    }

    /// The hit a reader at `line` meets first: the first on or after it, or the first of all when
    /// they are all behind.
    pub fn nearest(&self, line: LineIndex) -> HitIndex {
        let at = self.hits.partition_point(|hit| hit.line < line);
        match u32::try_from(at) {
            Ok(at) if (at as usize) < self.hits.len() => HitIndex(at),
            Ok(_) | Err(_) => HitIndex(0),
        }
    }

    /// The hits on `line`, and the number of the first of them.
    pub fn on_line(&self, line: LineIndex) -> (HitIndex, &[FindHit]) {
        let start = self.hits.partition_point(|hit| hit.line < line);
        let end = self.hits.partition_point(|hit| hit.line <= line);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &self.hits[start..end],
        )
    }
}

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
            if from >= to {
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

/// The line at the top that shows the hit on `hit`: where the view is when the hit is already in
/// the page, otherwise the hit a third of the way down the page.
pub(crate) fn top_for(hit: LineIndex, top: LineIndex, page: u32) -> LineIndex {
    let page = page.max(1);
    if hit.0 >= top.0 && hit.0 < top.0.saturating_add(page) {
        top
    } else {
        LineIndex(hit.0.saturating_sub(page / 3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_text::{ByteOffset, TokenSpan};

    fn hit(line: u32, from: u32, to: u32) -> FindHit {
        FindHit {
            line: LineIndex(line),
            from: ByteOffset(from),
            to: ByteOffset(to),
        }
    }

    fn span(class: TokenClass, text: &str) -> TokenSpan {
        TokenSpan {
            class,
            text: text.to_owned(),
        }
    }

    fn piece(class: TokenClass, text: &str, mark: Mark) -> Piece {
        Piece {
            class,
            text: text.to_owned(),
            mark,
        }
    }

    #[test]
    fn a_snippet_keeps_the_words_around_the_match_and_marks_where_it_is() {
        // name, line, match from, match to, the clip, the match in it
        let cases: Vec<(&str, &str, usize, usize, &str, &str)> = vec![
            (
                "a short line",
                "the needle here",
                4,
                10,
                "the needle here",
                "needle",
            ),
            (
                "leading space is dropped",
                "    the needle",
                8,
                14,
                "the needle",
                "needle",
            ),
            (
                "a character edge is respected",
                "caf\u{e9} needle",
                6,
                12,
                "caf\u{e9} needle",
                "needle",
            ),
            (
                "off a character edge marks nothing",
                "caf\u{e9} needle",
                4,
                6,
                "caf\u{e9} needle",
                "",
            ),
        ];
        for (name, line, from, to, text, matched) in cases {
            let snippet = clip(line, from, to);
            assert_eq!(snippet.text, text, "{name}: clip");
            assert_eq!(
                &snippet.text[snippet.matched.clone()],
                matched,
                "{name}: match"
            );
        }
        let long = format!("{} needle {}", "a".repeat(100), "b".repeat(120));
        let snippet = clip(&long, 101, 107);
        assert!(snippet.text.starts_with('\u{2026}') && snippet.text.ends_with('\u{2026}'));
        assert_eq!(&snippet.text[snippet.matched.clone()], "needle");
        assert!(snippet.text.len() < long.len());
    }

    #[test]
    fn a_line_is_cut_at_the_edges_of_its_hits_keeping_each_pieces_class() {
        use TokenClass::{Keyword, Plain};
        let line = TokenLine {
            number: LineIndex(0),
            spans: vec![span(Keyword, "pub fn"), span(Plain, " main()")],
        };
        // name, hits on the line, the current one's position, the pieces
        type Case = (&'static str, Vec<FindHit>, Option<usize>, Vec<Piece>);
        let cases: Vec<Case> = vec![
            (
                "no hit leaves the spans",
                vec![],
                None,
                vec![
                    piece(Keyword, "pub fn", Mark::Plain),
                    piece(Plain, " main()", Mark::Plain),
                ],
            ),
            (
                "a hit inside one span",
                vec![hit(0, 4, 6)],
                None,
                vec![
                    piece(Keyword, "pub ", Mark::Plain),
                    piece(Keyword, "fn", Mark::Hit),
                    piece(Plain, " main()", Mark::Plain),
                ],
            ),
            (
                "a hit across two spans is two pieces of one mark",
                vec![hit(0, 4, 9)],
                None,
                vec![
                    piece(Keyword, "pub ", Mark::Plain),
                    piece(Keyword, "fn", Mark::Hit),
                    piece(Plain, " ma", Mark::Hit),
                    piece(Plain, "in()", Mark::Plain),
                ],
            ),
            (
                "the current hit is marked apart from the others",
                vec![hit(0, 0, 3), hit(0, 7, 11)],
                Some(1),
                vec![
                    piece(Keyword, "pub", Mark::Hit),
                    piece(Keyword, " fn", Mark::Plain),
                    piece(Plain, " ", Mark::Plain),
                    piece(Plain, "main", Mark::Current),
                    piece(Plain, "()", Mark::Plain),
                ],
            ),
            (
                "a hit that is a whole span",
                vec![hit(0, 0, 6)],
                Some(0),
                vec![
                    piece(Keyword, "pub fn", Mark::Current),
                    piece(Plain, " main()", Mark::Plain),
                ],
            ),
        ];
        for (name, hits, current, want) in cases {
            assert_eq!(pieces(&line, &hits, current), want, "{name}");
        }
    }

    #[test]
    fn the_pieces_of_a_line_always_join_back_to_the_line() {
        use TokenClass::{Keyword, Plain};
        let line = TokenLine {
            number: LineIndex(0),
            spans: vec![span(Keyword, "café"), span(Plain, " au lait")],
        };
        let hits = [hit(0, 3, 7), hit(0, 8, 10)];
        let joined: String = pieces(&line, &hits, Some(0))
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        assert_eq!(joined, line.text());
    }

    #[test]
    fn hits_are_found_by_line_and_by_nearness() {
        let found = FoundHits::new(vec![hit(2, 0, 1), hit(2, 4, 5), hit(7, 0, 1), hit(9, 3, 4)]);
        // name, line, first hit on it and how many, the nearest hit from it
        const CASES: &[(&str, u32, u32, usize, u32)] = &[
            ("before every hit", 0, 0, 0, 0),
            ("on a line with two", 2, 0, 2, 0),
            ("between hits", 5, 2, 0, 2),
            ("on a line with one", 7, 2, 1, 2),
            ("past the last hit wraps to the first", 10, 4, 0, 0),
        ];
        for (name, line, first, count, nearest) in CASES {
            let (at, hits) = found.on_line(LineIndex(*line));
            assert_eq!((at.0, hits.len()), (*first, *count), "{name}: on the line");
            assert_eq!(
                found.nearest(LineIndex(*line)).0,
                *nearest,
                "{name}: nearest"
            );
        }
        assert_eq!(found.count(), HitCount(4));
        assert_eq!(FoundHits::default().nearest(LineIndex(3)), HitIndex(0));
    }

    #[test]
    fn a_hit_in_the_page_leaves_the_view_where_it_is_and_one_outside_brings_it_a_third_down() {
        // name, hit line, line at the top, lines in a page, the top after
        const CASES: &[(&str, u32, u32, u32, u32)] = &[
            ("in the page", 15, 10, 20, 10),
            ("first line of the page", 10, 10, 20, 10),
            ("just past the page", 30, 10, 20, 24),
            ("above the page", 3, 10, 20, 0),
            ("far below", 500, 10, 20, 494),
            ("a page of nothing still lands on the hit", 500, 10, 0, 500),
        ];
        for (name, hit, top, page, want) in CASES {
            assert_eq!(
                top_for(LineIndex(*hit), LineIndex(*top), *page),
                LineIndex(*want),
                "{name}"
            );
        }
    }
}
