//! Peeking at the fixtures: what each peek holds and the facts it lists, against recorded
//! expectations.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FormatKind, Input, Peek};
use anyview_text::{
    CodePeek, MarkdownPeek, PEEK_LINES, PlainPeek, RowLabel, TablePeek, Tally, TextCodec,
    TextError, TokenClass, TreePeek,
};
use support::{budget, expected, fixture, rows};

const PLENTY: u64 = 1_000_000;

fn peek<P: Peek>(name: &str, bytes: u64) -> Result<P::Peeked, P::Error> {
    let (src, sniffed) = fixture(name);
    P::peek(&Input::from(&src), &sniffed, &budget(bytes))
}

#[test]
fn plain_text_peeks_its_first_forty_lines_and_counts_them_all() {
    let peeked = peek::<PlainPeek>("notes.txt", PLENTY).unwrap();
    assert_eq!(peeked.lines.len(), PEEK_LINES);
    assert_eq!(peeked.lines[0], "note 1: remember to water the plants");
    assert_eq!(peeked.lines[39], "note 40: remember to water the plants");
    assert_eq!(peeked.total, Tally::Exact(60));
    assert_eq!(
        rows(&PlainPeek::facts(&peeked)),
        expected(&[
            ("kind", "Plain text"),
            ("lines", "60"),
            ("encoding", "UTF-8")
        ])
    );
}

#[test]
fn a_small_budget_reads_less_and_reports_a_lower_bound() {
    let peeked = peek::<PlainPeek>("notes.txt", 500).unwrap();
    assert_eq!(
        peeked.total,
        Tally::AtLeast(13),
        "whole lines inside 500 bytes"
    );
    assert_eq!(peeked.lines.len(), 13);
    assert_eq!(
        rows(&PlainPeek::facts(&peeked))[1],
        ("lines", "13+".to_owned())
    );
}

#[test]
fn legacy_and_utf16_files_are_decoded_for_the_peek() {
    let latin = peek::<PlainPeek>("latin1.txt", PLENTY).unwrap();
    assert_eq!(latin.encoding, TextCodec::Windows1252);
    // The first 4 KiB of the file is plain ASCII, so sniffing calls it text; the accents come after.
    assert_eq!(latin.lines.len(), 40);
    assert_eq!(&latin.lines[38..], ["café au lait", "naïve résumé"]);
    assert_eq!(
        rows(&PlainPeek::facts(&latin))[2],
        ("encoding", "Windows-1252".to_owned())
    );
    let wide = peek::<PlainPeek>("utf16.txt", PLENTY).unwrap();
    assert_eq!(wide.lines, ["héllo", "wörld"]);
    assert_eq!(
        rows(&PlainPeek::facts(&wide))[2],
        ("encoding", "UTF-16 LE".to_owned())
    );
}

#[test]
fn code_peeks_highlighted_lines_with_the_language_as_its_kind() {
    let peeked = peek::<CodePeek>("sample.rs", PLENTY).unwrap();
    assert_eq!(peeked.lines.len(), 16);
    assert!(
        peeked.lines[0]
            .spans
            .iter()
            .all(|s| s.class == TokenClass::Comment),
        "{:?}",
        peeked.lines[0]
    );
    let function = &peeked.lines[3];
    let class = |text: &str| {
        function
            .spans
            .iter()
            .find(|s| s.text.trim() == text)
            .map(|s| s.class)
    };
    assert_eq!(class("fn"), Some(TokenClass::Keyword));
    assert_eq!(class("add"), Some(TokenClass::Function));
    assert_eq!(
        peeked.lines[3].text(),
        "pub fn add(a: u32, b: u32) -> u32 {"
    );
    assert_eq!(
        rows(&CodePeek::facts(&peeked)),
        expected(&[
            ("kind", "Rust source"),
            ("lines", "16"),
            ("encoding", "UTF-8")
        ])
    );
}

#[test]
fn markdown_peeks_rendered_html_an_outline_and_a_title() {
    let peeked = peek::<MarkdownPeek>("readme.md", PLENTY).unwrap();
    let outline: Vec<(u8, &str, &str)> = peeked
        .outline
        .iter()
        .map(|h| (h.level.get(), h.text.as_str(), h.anchor.as_str()))
        .collect();
    assert_eq!(
        outline,
        [
            (1, "Anyview", "anyview"),
            (2, "Install", "install"),
            (2, "Install", "install-1")
        ]
    );
    assert!(peeked.html.contains("<table>"), "{}", peeked.html);
    assert!(
        peeked
            .html
            .contains("<input disabled=\"\" type=\"checkbox\" checked=\"\"/>")
    );
    assert!(
        peeked
            .html
            .contains("<span class=\"missing-image\">logo</span>"),
        "{}",
        peeked.html
    );
    assert!(peeked.html.contains("&lt;script&gt;"), "{}", peeked.html);
    assert!(!peeked.html.contains("<script"), "{}", peeked.html);
    assert_eq!(
        rows(&MarkdownPeek::facts(&peeked)),
        expected(&[
            ("kind", "Markdown document"),
            ("title", "Anyview"),
            ("lines", "22"),
            ("encoding", "UTF-8"),
        ])
    );
}

#[test]
fn a_csv_peeks_its_header_first_rows_and_shape() {
    let peeked = peek::<TablePeek>("people.csv", PLENTY).unwrap();
    assert_eq!(
        peeked.header.as_deref(),
        Some(&["name".to_owned(), "age".to_owned(), "city".to_owned()][..])
    );
    assert_eq!(peeked.rows.len(), PEEK_LINES);
    assert_eq!(peeked.rows[0], ["person1", "21", "city1"]);
    assert_eq!(
        rows(&TablePeek::facts(&peeked)),
        expected(&[
            ("kind", "CSV table"),
            ("rows", "50"),
            ("columns", "3"),
            ("encoding", "UTF-8"),
        ])
    );
}

#[test]
fn a_table_larger_than_the_budget_reports_a_lower_bound_of_rows() {
    let peeked = peek::<TablePeek>("people.csv", 300).unwrap();
    assert_eq!(peeked.total_rows, Tally::AtLeast(16));
    assert_eq!(
        rows(&TablePeek::facts(&peeked))[1],
        ("rows", "16+".to_owned())
    );
}

#[test]
fn a_tsv_uses_tabs() {
    let peeked = peek::<TablePeek>("scores.tsv", PLENTY).unwrap();
    assert_eq!(
        rows(&TablePeek::facts(&peeked)),
        expected(&[
            ("kind", "TSV table"),
            ("rows", "2"),
            ("columns", "2"),
            ("encoding", "UTF-8"),
        ])
    );
    assert_eq!(
        peeked.header.as_deref(),
        Some(&["id".to_owned(), "score".to_owned()][..])
    );
}

#[test]
fn json_peeks_the_top_level_of_its_tree() {
    let peeked = peek::<TreePeek>("config.json", PLENTY).unwrap();
    assert_eq!(peeked.root.preview, "{11 keys}");
    assert_eq!(peeked.top.len(), 11);
    assert_eq!(peeked.top[3].label, RowLabel::Key("scripts".to_owned()));
    assert_eq!(peeked.top[3].preview, "{2 keys}");
    assert_eq!(peeked.top[4].preview, "[2 items]");
    assert_eq!(
        rows(&TreePeek::facts(&peeked)),
        expected(&[
            ("kind", "JSON"),
            (
                "keys",
                "name, version, private, scripts, keywords, license, a, b and 3 more"
            ),
            ("encoding", "UTF-8"),
        ])
    );
}

#[test]
fn a_json_document_larger_than_the_budget_is_not_parsed() {
    assert_eq!(
        peek::<TreePeek>("config.json", 50),
        Err(TextError::JsonOverBudget)
    );
}

#[test]
fn json_lines_are_counted_exactly_or_as_a_lower_bound() {
    let whole = peek::<TreePeek>("events.jsonl", PLENTY).unwrap();
    assert_eq!(
        rows(&TreePeek::facts(&whole)),
        expected(&[
            ("kind", "JSON Lines"),
            ("entries", "100"),
            ("encoding", "UTF-8")
        ])
    );
    assert_eq!(whole.top.len(), PEEK_LINES);
    let start = peek::<TreePeek>("events.jsonl", 300).unwrap();
    assert_eq!(start.values, Some(Tally::AtLeast(11)));
    assert_eq!(
        rows(&TreePeek::facts(&start))[1],
        ("entries", "11+".to_owned())
    );
}

#[test]
fn a_peek_refuses_another_kind_and_an_empty_budget() {
    assert_eq!(
        peek::<PlainPeek>("sample.rs", PLENTY),
        Err(TextError::WrongKind {
            kind: FormatKind::Code
        })
    );
    assert_eq!(peek::<PlainPeek>("notes.txt", 0), Err(TextError::NoBudget));
}

#[test]
fn each_peek_is_for_one_kind() {
    assert_eq!(PlainPeek::KIND, FormatKind::PlainText);
    assert_eq!(CodePeek::KIND, FormatKind::Code);
    assert_eq!(MarkdownPeek::KIND, FormatKind::Markdown);
    assert_eq!(TablePeek::KIND, FormatKind::Table);
    assert_eq!(TreePeek::KIND, FormatKind::Tree);
}
