//! A find's hits as the text view draws them: the list a worker made, which of them are on a
//! line, how a highlighted line is cut where a hit starts and ends, and where the view scrolls
//! to show a hit. The stage machine holds only the count and the current index (`FindHits`);
//! this is the edge's side, the places themselves.

use crate::families::places::{CellHit, NodeHit};
use crate::{HitCount, HitIndex, SheetNo};
use anyview_core::{LineIndex, TreePath};
use anyview_text::{FindHit, Needle, TokenClass, TokenLine};

/// The words around a hit, for the palette's list: a clip of its line and where in the clip the
/// match is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    /// The clip.
    pub text: String,
    /// The match inside it, as byte offsets.
    pub matched: std::ops::Range<usize>,
    /// The character the match starts at in the line it was clipped from, counting from one; zero
    /// when the clip knows no line.
    pub column: usize,
}

/// Where the hits of a find are, by what the file is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Places {
    /// The matches in the lines of a text, in file order.
    Lines(Vec<FindHit>),
    /// The matches in the cells of a table's sheet, in file order.
    Cells {
        /// The sheet that was searched.
        sheet: SheetNo,
        /// The matches.
        hits: Vec<CellHit>,
    },
    /// The matches in the keys and values of a tree, in document order.
    Nodes(Vec<NodeHit>),
    /// The matches in the text of a rendered page, in reading order.
    Page {
        /// How many there are.
        count: u32,
        /// What marks them again when the page is drawn.
        needle: Needle,
    },
}

impl Default for Places {
    fn default() -> Self {
        Places::Lines(Vec::new())
    }
}

/// Every hit of a phrase in a file, in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FoundHits {
    places: Places,
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
            column: 0,
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
        column: line[..from].chars().count() + 1,
    }
}

impl FoundHits {
    /// The hits `hits`, which a search made in file order.
    #[must_use]
    pub fn new(hits: Vec<FindHit>) -> FoundHits {
        FoundHits {
            places: Places::Lines(hits),
            snippets: Vec::new(),
        }
    }

    /// The hits `hits` in the cells of `sheet` of a table, in file order.
    pub(crate) fn cells(sheet: SheetNo, hits: Vec<CellHit>) -> FoundHits {
        FoundHits {
            places: Places::Cells { sheet, hits },
            snippets: Vec::new(),
        }
    }

    /// The sheet the hits are in, when they are places in the cells of a table.
    pub(crate) fn sheet(&self) -> Option<SheetNo> {
        match &self.places {
            Places::Cells { sheet, .. } => Some(*sheet),
            Places::Lines(_) | Places::Nodes(_) | Places::Page { .. } => None,
        }
    }

    /// The hits `hits` in the nodes of a tree, in document order.
    pub(crate) fn nodes(hits: Vec<NodeHit>) -> FoundHits {
        FoundHits {
            places: Places::Nodes(hits),
            snippets: Vec::new(),
        }
    }

    /// `count` hits in the text of a rendered page, which `needle` marks.
    pub(crate) fn page(count: u32, needle: Needle) -> FoundHits {
        FoundHits {
            places: Places::Page { count, needle },
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
    #[must_use]
    pub fn count(&self) -> HitCount {
        let len = match &self.places {
            Places::Lines(hits) => hits.len(),
            Places::Cells { hits, .. } => hits.len(),
            Places::Nodes(hits) => hits.len(),
            Places::Page { count, .. } => *count as usize, // a u32 fits a usize
        };
        HitCount(u32::try_from(len).unwrap_or(u32::MAX))
    }

    /// The hit numbered `index`, when the hits are places in the lines of a text.
    #[must_use]
    pub fn get(&self, index: HitIndex) -> Option<&FindHit> {
        match &self.places {
            Places::Lines(hits) => hits.get(index.0 as usize),
            Places::Cells { .. } | Places::Nodes(_) | Places::Page { .. } => None,
        }
    }

    /// The hit numbered `index`, when the hits are places in the cells of a table.
    pub(crate) fn cell(&self, index: HitIndex) -> Option<&CellHit> {
        match &self.places {
            Places::Cells { hits, .. } => hits.get(index.0 as usize),
            Places::Lines(_) | Places::Nodes(_) | Places::Page { .. } => None,
        }
    }

    /// The hit numbered `index`, when the hits are places in the nodes of a tree.
    pub(crate) fn node(&self, index: HitIndex) -> Option<&NodeHit> {
        match &self.places {
            Places::Nodes(hits) => hits.get(index.0 as usize),
            Places::Lines(_) | Places::Cells { .. } | Places::Page { .. } => None,
        }
    }

    /// What marks the hits again in a rendered page, when they are places in one.
    pub(crate) fn page_needle(&self) -> Option<&Needle> {
        match &self.places {
            Places::Page { needle, .. } => Some(needle),
            Places::Lines(_) | Places::Cells { .. } | Places::Nodes(_) => None,
        }
    }

    /// The hit a reader at `line` meets first: the first on or after it, or the first of all when
    /// they are all behind. A table's hits are on its rows and a tree's or a page's have no line:
    /// the first of them.
    #[must_use]
    pub fn nearest(&self, line: LineIndex) -> HitIndex {
        let (at, len) = match &self.places {
            Places::Lines(hits) => (hits.partition_point(|hit| hit.line < line), hits.len()),
            Places::Cells { hits, .. } => {
                (hits.partition_point(|hit| hit.row < line.0), hits.len())
            }
            Places::Nodes(_) | Places::Page { .. } => return HitIndex(0),
        };
        match u32::try_from(at) {
            Ok(at) if (at as usize) < len => HitIndex(at),
            Ok(_) | Err(_) => HitIndex(0),
        }
    }

    /// The hits on `line`, and the number of the first of them.
    #[must_use]
    pub fn on_line(&self, line: LineIndex) -> (HitIndex, &[FindHit]) {
        let Places::Lines(hits) = &self.places else {
            return (HitIndex(0), &[]);
        };
        let start = hits.partition_point(|hit| hit.line < line);
        let end = hits.partition_point(|hit| hit.line <= line);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &hits[start..end],
        )
    }

    /// The hits in the cells of `row`, and the number of the first of them.
    pub(crate) fn in_row(&self, row: u32) -> (HitIndex, &[CellHit]) {
        let Places::Cells { hits, .. } = &self.places else {
            return (HitIndex(0), &[]);
        };
        let start = hits.partition_point(|hit| hit.row < row);
        let end = hits.partition_point(|hit| hit.row <= row);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &hits[start..end],
        )
    }

    /// The hits in the node at `path`, and the number of the first of them.
    pub(crate) fn at_node(&self, path: &TreePath) -> (HitIndex, &[NodeHit]) {
        let Places::Nodes(hits) = &self.places else {
            return (HitIndex(0), &[]);
        };
        let start = hits.partition_point(|hit| hit.path < *path);
        let end = hits.partition_point(|hit| hit.path <= *path);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &hits[start..end],
        )
    }
}

/// Where `hit` is, as the palette says it: the line, and for a line with more than one hit the
/// column too, so the rows of those hits tell apart.
pub(super) fn hit_place(found: &FoundHits, hit: &FindHit, snippet: &Snippet) -> String {
    let (_, on_line) = found.on_line(hit.line);
    let line = hit.line.0 + 1;
    if on_line.len() > 1 && snippet.column > 0 {
        format!("Line {line}, column {}", snippet.column)
    } else {
        format!("Line {line}")
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
    use crate::families::places::NodePart;
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
    fn two_hits_on_one_line_are_told_apart_by_their_columns() {
        let found = FoundHits::new(vec![hit(2, 0, 3), hit(2, 10, 13), hit(5, 4, 7)]);
        let on = |column: usize| Snippet {
            column,
            ..Snippet::default()
        };
        // name, the hit, its snippet's column, the place
        let cases: Vec<(&str, usize, usize, &str)> = vec![
            ("the first of two", 0, 1, "Line 3, column 1"),
            ("the second of two", 1, 11, "Line 3, column 11"),
            ("alone on its line", 2, 5, "Line 6"),
            ("a clip that knows no column", 0, 0, "Line 3"),
        ];
        for (name, at, column, want) in cases {
            let hit = found.get(HitIndex(at as u32)).unwrap();
            assert_eq!(hit_place(&found, hit, &on(column)), want, "{name}");
        }
    }

    #[test]
    fn a_snippet_knows_the_column_its_match_starts_at() {
        // name, line, match from, match to, column
        let cases: Vec<(&str, &str, usize, usize, usize)> = vec![
            ("at the start", "needle here", 0, 6, 1),
            ("after words", "the needle", 4, 10, 5),
            ("after a wide character", "caf\u{e9} needle", 6, 12, 6),
        ];
        for (name, line, from, to, column) in cases {
            assert_eq!(clip(line, from, to).column, column, "{name}");
        }
    }

    #[test]
    fn hits_of_a_table_a_tree_and_a_page_are_counted_and_found_in_their_own_places() {
        let cells = FoundHits::cells(
            SheetNo(0),
            vec![
                CellHit {
                    row: 1,
                    column: 0,
                    from: 0,
                    to: 1,
                },
                CellHit {
                    row: 1,
                    column: 2,
                    from: 0,
                    to: 1,
                },
                CellHit {
                    row: 4,
                    column: 1,
                    from: 2,
                    to: 3,
                },
            ],
        );
        assert_eq!(cells.count(), HitCount(3));
        assert_eq!(cells.sheet(), Some(SheetNo(0)));
        assert_eq!(cells.in_row(1).0, HitIndex(0));
        assert_eq!(cells.in_row(1).1.len(), 2);
        assert_eq!(cells.in_row(4).0, HitIndex(2));
        assert!(cells.in_row(2).1.is_empty());
        assert_eq!(
            cells.nearest(LineIndex(2)),
            HitIndex(2),
            "the first hit on or after the row"
        );
        assert_eq!(
            cells.nearest(LineIndex(9)),
            HitIndex(0),
            "past the last wraps"
        );
        assert_eq!(cells.get(HitIndex(0)), None, "no line to scroll to");
        let at = |steps: &[u32]| steps.iter().fold(TreePath::root(), |p, s| p.child(*s));
        let node = |steps: &[u32], part| NodeHit {
            path: at(steps),
            part,
            from: 0,
            to: 1,
            trail: String::new(),
        };
        let nodes = FoundHits::nodes(vec![
            node(&[0], NodePart::Key),
            node(&[0], NodePart::Value),
            node(&[0, 3], NodePart::Value),
        ]);
        assert_eq!(nodes.count(), HitCount(3));
        assert_eq!(nodes.at_node(&at(&[0])).0, HitIndex(0));
        assert_eq!(nodes.at_node(&at(&[0])).1.len(), 2);
        assert_eq!(nodes.at_node(&at(&[0, 3])).0, HitIndex(2));
        assert!(nodes.at_node(&at(&[1])).1.is_empty());
        let page = FoundHits::page(7, Needle::new("cat").unwrap());
        assert_eq!(page.count(), HitCount(7));
        assert!(page.page_needle().is_some());
        assert!(FoundHits::default().page_needle().is_none());
    }

    #[test]
    fn a_cell_is_cut_at_its_hits_and_always_joins_back() {
        use Mark::{Current, Hit, Plain};
        // name, text, hits, the pieces
        type Case = (
            &'static str,
            &'static str,
            Vec<(std::ops::Range<usize>, Mark)>,
            Vec<(&'static str, Mark)>,
        );
        let cases: Vec<Case> = vec![
            ("no hit", "Lima", vec![], vec![("Lima", Plain)]),
            ("an empty cell", "", vec![], vec![("", Plain)]),
            (
                "a hit in the middle",
                "Lima, Peru",
                vec![(0..4, Hit)],
                vec![("Lima", Hit), (", Peru", Plain)],
            ),
            (
                "two hits, the second current",
                "a cat, a cat",
                vec![(2..5, Hit), (9..12, Current)],
                vec![
                    ("a ", Plain),
                    ("cat", Hit),
                    (", a ", Plain),
                    ("cat", Current),
                ],
            ),
            (
                "a range off a character edge is left out",
                "caf\u{e9}",
                vec![(4..5, Hit)],
                vec![("caf\u{e9}", Plain)],
            ),
            (
                "a range past the end is left out",
                "ab",
                vec![(1..9, Hit)],
                vec![("ab", Plain)],
            ),
        ];
        for (name, text, hits, want) in cases {
            let got = cut(text, &hits);
            let want: Vec<(String, Mark)> = want
                .into_iter()
                .map(|(piece, mark)| (piece.to_owned(), mark))
                .collect();
            assert_eq!(got, want, "{name}");
            let joined: String = got.iter().map(|(piece, _)| piece.as_str()).collect();
            assert_eq!(joined, text, "{name}: joined");
        }
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
