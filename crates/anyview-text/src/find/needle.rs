//! The phrase searched for and one place it was found.

use anyview_core::{CaseMatch, FindOptions, LineIndex, WordMatch};

/// A byte position inside one line of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteOffset(pub u32);

/// One occurrence of the phrase: a line, and the bytes of it the phrase covers (`from` up to,
/// not including, `to`). Hits come out in file order and never overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FindHit {
    /// The line it is on, counting from zero.
    pub line: LineIndex,
    /// Where it starts in the line's text.
    pub from: ByteOffset,
    /// Where it ends in the line's text.
    pub to: ByteOffset,
}

/// A phrase to search for, with its lower-case form made once. It is never empty: an empty phrase finds nothing
/// useful, so there is no way to hold one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Needle {
    exact: Vec<char>,
    lower: Vec<char>,
    options: FindOptions,
}

impl Needle {
    /// The phrase `text`, or `None` when it is empty. It ignores case and may be part of a longer
    /// word until [`Needle::with_options`] says otherwise.
    #[must_use]
    pub fn new(text: &str) -> Option<Needle> {
        let exact: Vec<char> = text.chars().collect();
        let lower: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
        (!exact.is_empty()).then_some(Needle {
            exact,
            lower,
            options: FindOptions::default(),
        })
    }

    /// This phrase matched as `options` say.
    #[must_use]
    pub fn with_options(self, options: FindOptions) -> Needle {
        Needle { options, ..self }
    }

    /// Every hit in `text`, which is the line numbered `line`.
    pub fn hits_in(&self, line: LineIndex, text: &str) -> Vec<FindHit> {
        let mut hits = Vec::new();
        let mut at = 0;
        while at < text.len() {
            let Some(rest) = text.get(at..) else { break };
            match self.matched(rest) {
                Some(used) if self.stands_alone(text, at, at + used) => {
                    hits.push(FindHit {
                        line,
                        from: offset(at),
                        to: offset(at + used),
                    });
                    at += used;
                }
                Some(_) | None => at += rest.chars().next().map_or(1, char::len_utf8),
            }
        }
        hits
    }

    /// How many bytes of `rest` the phrase covers when it starts there, or `None` when it does
    /// not. A phrase must end between two characters: lower-casing one character can give
    /// several, and a phrase that stops inside them is not a match.
    fn matched(&self, rest: &str) -> Option<usize> {
        let mut want = match self.options.case {
            CaseMatch::Ignore => self.lower.iter(),
            CaseMatch::Exact => self.exact.iter(),
        };
        let mut used = 0;
        for c in rest.chars() {
            match self.options.case {
                CaseMatch::Ignore => {
                    for lower in c.to_lowercase() {
                        if want.next() != Some(&lower) {
                            return None;
                        }
                    }
                }
                CaseMatch::Exact => {
                    if want.next() != Some(&c) {
                        return None;
                    }
                }
            }
            used += c.len_utf8();
            if want.len() == 0 {
                return Some(used);
            }
        }
        None
    }

    /// Whether the bytes `from..to` of `text` may be a hit: any, unless only whole words are
    /// wanted, and then a hit that starts or ends in a word must not run into the word beside it.
    /// A hit that starts or ends in punctuation (`fn(`, `.rs`) has no word to run into there.
    fn stands_alone(&self, text: &str, from: usize, to: usize) -> bool {
        match self.options.words {
            WordMatch::Part => true,
            WordMatch::Whole => {
                let hit = text.get(from..to).unwrap_or_default();
                let before = text.get(..from).and_then(|head| head.chars().next_back());
                let after = text.get(to..).and_then(|tail| tail.chars().next());
                !(hit.chars().next().is_some_and(in_word) && before.is_some_and(in_word))
                    && !(hit.chars().next_back().is_some_and(in_word) && after.is_some_and(in_word))
            }
        }
    }
}

/// Whether `c` belongs to a word: a letter or digit of any script, or the underscore that joins
/// the words of a name.
fn in_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn offset(bytes: usize) -> ByteOffset {
    ByteOffset(u32::try_from(bytes).unwrap_or(u32::MAX))
}
