use super::*;
use anyview_core::{CaseMatch, FindOptions, WordMatch};

fn hit(line: u32, from: u32, to: u32) -> FindHit {
    FindHit {
        line: LineIndex(line),
        from: ByteOffset(from),
        to: ByteOffset(to),
    }
}

const LOOSE: FindOptions = FindOptions::DEFAULT;
const EXACT: FindOptions = FindOptions::DEFAULT.with_case(CaseMatch::Exact);
const WHOLE: FindOptions = FindOptions::DEFAULT.with_words(WordMatch::Whole);
const BOTH: FindOptions = EXACT.with_words(WordMatch::Whole);

#[test]
fn a_phrase_is_found_in_a_line_under_each_option_and_never_overlaps() {
    // name, phrase, options, line text, hits as (from, to)
    type Case = (
        &'static str,
        &'static str,
        FindOptions,
        &'static str,
        &'static [(u32, u32)],
    );
    const CASES: &[Case] = &[
        ("once", "fn", LOOSE, "pub fn main", &[(4, 6)]),
        ("case is ignored", "FN", LOOSE, "pub fn main", &[(4, 6)]),
        ("twice", "ab", LOOSE, "ab cab", &[(0, 2), (4, 6)]),
        ("overlap takes the first", "aa", LOOSE, "aaa", &[(0, 2)]),
        ("none", "zz", LOOSE, "pub fn main", &[]),
        (
            "a multi-byte letter keeps its bytes",
            "é",
            LOOSE,
            "café au lait",
            &[(3, 5)],
        ),
        (
            "the phrase has spaces",
            "au l",
            LOOSE,
            "café au lait",
            &[(6, 10)],
        ),
        ("longer than the line", "abcdef", LOOSE, "abc", &[]),
        ("an empty line", "a", LOOSE, "", &[]),
        (
            "exact case finds its case",
            "Fox",
            EXACT,
            "Fox fox",
            &[(0, 3)],
        ),
        ("exact case misses the other", "FOX", EXACT, "Fox fox", &[]),
        (
            "exact case tells a capital apart in any script",
            "Éa",
            EXACT,
            "éa Éa",
            &[(4, 7)],
        ),
        (
            "a whole word is not the start of a longer one",
            "cat",
            WHOLE,
            "cat concat cats cat_x cat",
            &[(0, 3), (22, 25)],
        ),
        (
            "punctuation and the line's ends bound a word",
            "cat",
            WHOLE,
            "(cat), cat. \"cat\"",
            &[(1, 4), (7, 10), (13, 16)],
        ),
        (
            "a digit or an underscore joins a word",
            "id",
            WHOLE,
            "id2 2id _id id_ id",
            &[(16, 18)],
        ),
        (
            "a non-ASCII letter joins a word",
            "caf",
            WHOLE,
            "café caf",
            &[(6, 9)],
        ),
        (
            "a non-ASCII word is whole between spaces and punctuation",
            "é",
            WHOLE,
            "é, (é) aé",
            &[(0, 2), (5, 7)],
        ),
        (
            "a hit that begins or ends in punctuation has no word to join there",
            ".rs",
            WHOLE,
            "a.rs",
            &[(1, 4)],
        ),
        (
            "a rejected spot does not hide the next one",
            "ab",
            WHOLE,
            "abab ab",
            &[(5, 7)],
        ),
        (
            "case and whole word together",
            "Fox",
            BOTH,
            "Fox Foxes fox (Fox)",
            &[(0, 3), (15, 18)],
        ),
    ];
    for (name, phrase, options, text, want) in CASES {
        let needle = Needle::new(phrase).unwrap().with_options(*options);
        let got: Vec<(u32, u32)> = needle
            .hits_in(LineIndex(0), text)
            .iter()
            .map(|h| (h.from.0, h.to.0))
            .collect();
        assert_eq!(&got, want, "{name}");
    }
}

#[test]
fn an_empty_phrase_is_no_needle() {
    assert_eq!(Needle::new(""), None);
}

#[test]
fn a_phrase_that_stops_inside_a_letters_lowercase_is_not_a_match() {
    // "İ" lower-cases to "i" and a combining dot: "i" alone matches the letter's first half only.
    let needle = Needle::new("i").unwrap();
    assert_eq!(needle.hits_in(LineIndex(0), "İ"), vec![]);
}

#[test]
fn a_file_is_searched_line_by_line_in_order() {
    let text = b"alpha\nBeta alpha\n\ngamma ALPHA alpha\n".to_vec();
    let lines = TextLines::open(text).unwrap();
    let needle = Needle::new("alpha").unwrap();
    assert_eq!(
        lines.find(&needle, &Stop::new()).unwrap(),
        vec![hit(0, 0, 5), hit(1, 5, 10), hit(3, 6, 11), hit(3, 12, 17)]
    );
}

#[test]
fn a_search_keeps_no_more_than_the_most_it_may() {
    let many = "a\n".repeat(MAX_HITS + 50);
    let lines = TextLines::open(many.into_bytes()).unwrap();
    let hits = lines
        .find(&Needle::new("a").unwrap(), &Stop::new())
        .unwrap();
    assert_eq!(hits.len(), MAX_HITS);
    assert_eq!(
        hits.last().map(|h| h.line),
        Some(LineIndex(MAX_HITS as u32 - 1))
    );
}

#[test]
fn a_search_that_was_stopped_reads_nothing_more() {
    let text = b"alpha\nalpha\n".to_vec();
    let lines = TextLines::open(text).unwrap();
    let stop = Stop::new();
    stop.request();
    let hits = lines.find(&Needle::new("alpha").unwrap(), &stop).unwrap();
    assert_eq!(hits, vec![]);
}
