//! Tests of the snippets, the marks that cut a line and the places of the hits.

use super::found::{FoundHits, hit_place, top_for};
use super::marks::{Mark, Piece, cut, pieces};
use super::snippet::{Snippet, clip};
use crate::families::places::{CellHit, NodeHit};
use crate::{HitCount, HitIndex, SheetNo};
use anyview_core::{LineIndex, TreePath};
use anyview_text::{FindHit, Needle, TokenClass, TokenLine};

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
