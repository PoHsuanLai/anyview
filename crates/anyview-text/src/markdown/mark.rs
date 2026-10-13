//! A find in a rendered page: the hits are the places the phrase occurs in the page's text (its
//! tags are not text, and an entity is the character it stands for), counted in reading order,
//! and marking them wraps each in a `<mark>`, the current one with `data-current`. The rendered
//! body is what the viewer shows, so this is what a person sees to search, not the Markdown
//! source behind it.

use crate::find::{MAX_HITS, Needle};
use anyview_core::LineIndex;
use std::ops::Range;

/// One hit in a page: the text of the run it is in, and where in that text the match is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageHit {
    /// The text of the run of the page the hit is in, entities read.
    pub text: String,
    /// The match inside it, as byte offsets.
    pub matched: Range<usize>,
}

/// What searching a page found: how many hits, and the first few of them in full.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageHits {
    /// How many hits there are, at most [`MAX_HITS`].
    pub count: u32,
    /// The first hits, in reading order.
    pub first: Vec<PageHit>,
}

/// The byte ranges of `html` that are text and not tags, in order.
fn text_runs(html: &str) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while let Some(open) = html.get(at..).and_then(|rest| rest.find('<')) {
        let open = at + open;
        if open > start {
            runs.push(start..open);
        }
        match html.get(open..).and_then(|rest| rest.find('>')) {
            Some(close) => {
                at = open + close + 1;
                start = at;
            }
            None => return runs,
        }
    }
    if start < html.len() {
        runs.push(start..html.len());
    }
    runs
}

/// The character an entity at the start of `rest` stands for, and how many bytes it takes.
fn entity(rest: &str) -> Option<(char, usize)> {
    const ENTITIES: [(&str, char); 5] = [
        ("&amp;", '&'),
        ("&lt;", '<'),
        ("&gt;", '>'),
        ("&quot;", '"'),
        ("&#39;", '\''),
    ];
    ENTITIES
        .iter()
        .find(|(name, _)| rest.starts_with(name))
        .map(|(name, character)| (*character, name.len()))
}

/// `escaped` with its entities read, and for each byte of that text (and one past the end) where
/// it began in `escaped`.
fn unescaped(escaped: &str) -> (String, Vec<usize>) {
    let mut plain = String::with_capacity(escaped.len());
    let mut began = Vec::with_capacity(escaped.len() + 1);
    let mut offset = 0;
    while let Some(next) = escaped.get(offset..).and_then(|rest| rest.chars().next()) {
        let (character, used) = if next == '&' {
            escaped
                .get(offset..)
                .and_then(entity)
                .unwrap_or((next, next.len_utf8()))
        } else {
            (next, next.len_utf8())
        };
        began.extend(std::iter::repeat_n(offset, character.len_utf8()));
        plain.push(character);
        offset += used;
    }
    began.push(offset);
    (plain, began)
}

/// Where a hit is: the run of the page it is in, the plain text of that run, where each byte of
/// the plain text began in the run, and the match in the plain text.
struct Place<'a> {
    run: &'a Range<usize>,
    began: &'a [usize],
    text: &'a str,
    matched: Range<usize>,
}

/// Call `visit` with every hit of `needle` in the text of `html`, in reading order and the first
/// [`MAX_HITS`] of them, as numbered.
fn each_hit(html: &str, needle: &Needle, mut visit: impl FnMut(usize, &Place<'_>)) {
    let mut number = 0;
    for run in text_runs(html) {
        let (plain, began) = unescaped(html.get(run.clone()).unwrap_or_default());
        for hit in needle.hits_in(LineIndex(0), &plain) {
            if number >= MAX_HITS {
                return;
            }
            let matched = (hit.from.0 as usize)..(hit.to.0 as usize); // a u32 fits a usize
            visit(
                number,
                &Place {
                    run: &run,
                    began: &began,
                    text: &plain,
                    matched,
                },
            );
            number += 1;
        }
    }
}

/// The hits of `needle` in `html`, of which the first `keep` are kept whole.
#[must_use]
pub fn page_hits(html: &str, needle: &Needle, keep: usize) -> PageHits {
    let mut found = PageHits::default();
    each_hit(html, needle, |_, place| {
        found.count = found.count.saturating_add(1);
        if found.first.len() < keep {
            found.first.push(PageHit {
                text: place.text.to_owned(),
                matched: place.matched.clone(),
            });
        }
    });
    found
}

/// `html` with every hit of `needle` wrapped in `<mark>`, the one numbered `current` (counting
/// from zero, as [`page_hits`] counts) marked `data-current`.
#[must_use]
pub fn mark_page(html: &str, needle: &Needle, current: Option<usize>) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut copied = 0;
    each_hit(html, needle, |number, place| {
        let end = place.run.end - place.run.start;
        let at = |offset: usize| place.run.start + place.began.get(offset).copied().unwrap_or(end);
        let (from, to) = (at(place.matched.start), at(place.matched.end));
        if from < copied || to < from {
            return;
        }
        out.push_str(html.get(copied..from).unwrap_or_default());
        out.push_str(if Some(number) == current {
            "<mark data-current=\"yes\">"
        } else {
            "<mark>"
        });
        out.push_str(html.get(from..to).unwrap_or_default());
        out.push_str("</mark>");
        copied = to;
    });
    out.push_str(html.get(copied..).unwrap_or_default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{CaseMatch, FindOptions};

    fn needle(text: &str) -> Needle {
        Needle::new(text).unwrap()
    }

    #[test]
    fn hits_are_in_the_text_and_never_in_a_tag() {
        // name, html, phrase, how many hits
        const CASES: &[(&str, &str, &str, u32)] = &[
            ("in a paragraph", "<p>the cat sat</p>", "cat", 1),
            ("not in an attribute", "<a href=\"cat\">dog</a>", "cat", 0),
            ("not in a tag name", "<em>strong</em>", "em", 0),
            ("across two paragraphs", "<p>a cat</p><p>cat</p>", "cat", 2),
            ("twice in a run", "<p>cat cat</p>", "cat", 2),
            (
                "an entity is its character",
                "<p>fish &amp; chips</p>",
                "& ",
                1,
            ),
            (
                "the letters of an entity are not text",
                "<p>fish &amp; chips</p>",
                "amp",
                0,
            ),
        ];
        for (name, html, phrase, want) in CASES {
            assert_eq!(page_hits(html, &needle(phrase), 10).count, *want, "{name}");
        }
    }

    #[test]
    fn the_first_hits_are_kept_with_the_words_they_are_in() {
        let found = page_hits("<p>one cat</p><p>two cats</p>", &needle("cat"), 1);
        assert_eq!(found.count, 2);
        assert_eq!(
            found.first,
            [PageHit {
                text: "one cat".to_owned(),
                matched: 4..7
            }]
        );
    }

    #[test]
    fn marking_wraps_each_hit_and_singles_out_the_current_one() {
        // name, html, phrase, current, marked
        const CASES: &[(&str, &str, &str, Option<usize>, &str)] = &[
            (
                "none current",
                "<p>a cat, a Cat</p>",
                "cat",
                None,
                "<p>a <mark>cat</mark>, a <mark>Cat</mark></p>",
            ),
            (
                "the second is current",
                "<p>cat</p><p>cat</p>",
                "cat",
                Some(1),
                "<p><mark>cat</mark></p><p><mark data-current=\"yes\">cat</mark></p>",
            ),
            (
                "an entity in the run is kept as written",
                "<p>a &amp; b cat</p>",
                "cat",
                None,
                "<p>a &amp; b <mark>cat</mark></p>",
            ),
            (
                "a hit that is an entity wraps it whole",
                "<p>a &lt; b</p>",
                "<",
                None,
                "<p>a <mark>&lt;</mark> b</p>",
            ),
            ("no hits is the page", "<p>a</p>", "cat", None, "<p>a</p>"),
        ];
        for (name, html, phrase, current, want) in CASES {
            assert_eq!(mark_page(html, &needle(phrase), *current), *want, "{name}");
        }
    }

    #[test]
    fn marking_follows_the_options_of_the_needle() {
        let exact = needle("Cat").with_options(FindOptions::default().with_case(CaseMatch::Exact));
        assert_eq!(
            mark_page("<p>cat Cat</p>", &exact, None),
            "<p>cat <mark>Cat</mark></p>"
        );
    }
}
