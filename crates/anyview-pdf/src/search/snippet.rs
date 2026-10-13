//! The words around a hit, for the list of places a find offers.

use std::ops::Range;

/// How many characters of the page's text a clip keeps before the match.
const BEFORE: usize = 32;

/// How many characters of the page's text a clip keeps after the match.
const AFTER: usize = 72;

/// The words around a hit: a clip of the page's text on one line, and where in it the match is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    /// The clip. It stops at the line the match is on, with an ellipsis where it was cut short;
    /// line breaks and runs of space inside it are single spaces.
    pub text: String,
    /// The match inside the clip, as byte offsets.
    pub matched: Range<usize>,
}

/// What one character of the page's text becomes in a clip.
enum Clipped {
    /// It is dropped: a soft hyphen or another control character.
    Gone,
    /// It is a space, and runs of them are one.
    Space,
    /// It stays.
    Kept(char),
}

fn clipped(c: char) -> Clipped {
    if c == '\u{00AD}' {
        Clipped::Gone
    } else if c.is_whitespace() {
        Clipped::Space
    } else if c.is_control() {
        Clipped::Gone
    } else {
        Clipped::Kept(c)
    }
}

fn is_break(c: char) -> bool {
    matches!(c, '\n' | '\r')
}

impl Snippet {
    /// The clip of `text` around the characters `from..to`, which are the match: the line it starts
    /// on, up to a few words either side. A match that runs over a line break keeps both lines.
    pub(crate) fn around(text: &[char], from: usize, to: usize) -> Snippet {
        let to = to.min(text.len());
        let from = from.min(to);
        let joins = |at: usize| text.get(at).is_some_and(|c| !is_break(*c));
        let mut start = from;
        while start > 0 && from - start < BEFORE && joins(start - 1) {
            start -= 1;
        }
        let cut_before = start > 0 && joins(start - 1);
        let mut end = to;
        while end < text.len() && end - to < AFTER && joins(end) {
            end += 1;
        }
        let cut_after = end < text.len() && joins(end);

        let mut body = String::new();
        let (mut open, mut close) = (0, None);
        let mut spaced = true;
        for (at, c) in (start..end).zip(text.get(start..end).unwrap_or_default()) {
            if at == from {
                open = body.len();
            }
            if at == to {
                close = Some(body.len());
            }
            match clipped(*c) {
                Clipped::Gone => {}
                Clipped::Space => {
                    if !spaced {
                        body.push(' ');
                        spaced = true;
                    }
                }
                Clipped::Kept(c) => {
                    body.push(c);
                    spaced = false;
                }
            }
        }
        if body.ends_with(' ') {
            body.pop();
        }
        let close = close.unwrap_or(body.len()).min(body.len());
        let open = open.min(close);
        let head = if cut_before { "\u{2026}" } else { "" };
        let tail = if cut_after { "\u{2026}" } else { "" };
        Snippet {
            text: format!("{head}{body}{tail}"),
            matched: head.len() + open..head.len() + close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn around(text: &str, needle: &str) -> Snippet {
        let byte = text.find(needle).expect("the needle is in the text");
        let from = text[..byte].chars().count();
        let chars: Vec<char> = text.chars().collect();
        Snippet::around(&chars, from, from + needle.chars().count())
    }

    #[test]
    fn a_clip_is_the_line_of_the_match_on_one_line_with_the_match_located() {
        // name, page text, match, clip, the match in the clip
        const CASES: &[(&str, &str, &str, &str, &str)] = &[
            (
                "a short line",
                "The quick brown fox jumps",
                "fox",
                "The quick brown fox jumps",
                "fox",
            ),
            (
                "the line ends at a break",
                "Chapter One\nThe fox ran\nAfter",
                "fox",
                "The fox ran",
                "fox",
            ),
            (
                "a phrase over a break keeps both lines",
                "It ends with dog.\nChapter Two\nMore",
                "dog.\nChapter",
                "It ends with dog. Chapter Two",
                "dog. Chapter",
            ),
            (
                "soft hyphens go and spaces are one",
                "note\u{ad}book  and\u{a0}\u{a0}fox",
                "fox",
                "notebook and fox",
                "fox",
            ),
        ];
        for (name, page, needle, clip, matched) in CASES {
            let got = around(page, needle);
            assert_eq!(&got.text, clip, "{name}");
            assert_eq!(&got.text[got.matched.clone()], *matched, "{name}");
        }
    }

    #[test]
    fn a_long_line_is_cut_either_side_with_an_ellipsis() {
        let page = format!("{} fox {}", "a".repeat(100), "b".repeat(100));
        let got = around(&page, "fox");
        assert!(got.text.starts_with('\u{2026}'), "{}", got.text);
        assert!(got.text.ends_with('\u{2026}'), "{}", got.text);
        assert_eq!(&got.text[got.matched.clone()], "fox");
        assert!(
            got.text.chars().count() < BEFORE + AFTER + 8,
            "{}",
            got.text
        );
    }
}
