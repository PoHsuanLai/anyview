//! The outline of a Markdown document: its headings, each with the anchor the rendered HTML
//! carries as its `id`.

use std::collections::HashMap;

/// A heading's depth, 1 (`#`) to 6 (`######`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HeadingLevel(u8);

impl HeadingLevel {
    /// The level of a heading written with `hashes` `#` characters, held to 1 through 6.
    pub fn clamped(hashes: u8) -> Self {
        HeadingLevel(hashes.clamp(1, 6))
    }

    /// The number of `#` characters, 1 to 6.
    pub fn get(self) -> u8 {
        self.0
    }
}

/// The `id` of a heading in the rendered HTML, and the fragment (`#anchor`) that jumps to it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Anchor(String);

impl Anchor {
    /// The id, without the `#`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One heading of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// How deep it is.
    pub level: HeadingLevel,
    /// Its text, markup removed.
    pub text: String,
    /// Where it is in the rendered HTML.
    pub anchor: Anchor,
}

/// `text` as an anchor, the way GitHub makes one: lower-case, letters, digits, `-` and `_` kept,
/// spaces become `-`, everything else dropped. Text with nothing left is `section`.
fn slug(text: &str) -> String {
    let slug: String = text
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect();
    if slug.is_empty() {
        "section".to_owned()
    } else {
        slug
    }
}

/// Gives each heading text an anchor that no earlier one has: the second `Setup` is `setup-1`, the
/// third `setup-2`.
#[derive(Debug, Default)]
pub(super) struct Anchors {
    taken: HashMap<String, u32>,
}

impl Anchors {
    pub(super) fn next(&mut self, text: &str) -> Anchor {
        let base = slug(text);
        let mut candidate = base.clone();
        loop {
            let uses = self.taken.entry(candidate.clone()).or_insert(0);
            if *uses == 0 {
                *uses = 1;
                return Anchor(candidate);
            }
            let n = *uses;
            *uses += 1;
            candidate = format!("{base}-{n}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_become_github_style_slugs() {
        const CASES: &[(&str, &str, &str)] = &[
            ("words", "Hello World", "hello-world"),
            ("punctuation dropped", "Hello, World!", "hello-world"),
            (
                "hyphens and underscores kept",
                "snake_case-name",
                "snake_case-name",
            ),
            ("surrounding space trimmed", "  Padded  ", "padded"),
            ("inner spaces are not collapsed", "a  b", "a--b"),
            ("digits", "Step 2: Run", "step-2-run"),
            ("unicode letters", "Über Café", "über-café"),
            ("nothing left", "!!!", "section"),
            ("empty", "", "section"),
        ];
        for (name, text, want) in CASES {
            assert_eq!(slug(text), *want, "{name}");
        }
    }

    #[test]
    fn repeated_headings_get_numbered_anchors_that_never_collide() {
        let mut anchors = Anchors::default();
        let got: Vec<String> = ["Setup", "Usage", "Setup", "Setup", "setup-1"]
            .iter()
            .map(|text| anchors.next(text).as_str().to_owned())
            .collect();
        assert_eq!(got, ["setup", "usage", "setup-1", "setup-2", "setup-1-1"]);
    }

    #[test]
    fn levels_are_held_to_one_through_six() {
        // name, given, level
        const CASES: &[(&str, u8, u8)] = &[
            ("zero", 0, 1),
            ("one", 1, 1),
            ("three", 3, 3),
            ("six", 6, 6),
            ("seven", 7, 6),
        ];
        for (name, hashes, want) in CASES {
            assert_eq!(HeadingLevel::clamped(*hashes).get(), *want, "{name}");
        }
    }
}
