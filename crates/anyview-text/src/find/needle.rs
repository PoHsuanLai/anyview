//! The phrase searched for and one place it was found.

use anyview_core::LineIndex;

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

/// A phrase to search for, lower-cased once. It is never empty: an empty phrase finds nothing
/// useful, so there is no way to hold one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Needle {
    lower: Vec<char>,
}

impl Needle {
    /// The phrase `text`, or `None` when it is empty.
    pub fn new(text: &str) -> Option<Needle> {
        let lower: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
        (!lower.is_empty()).then_some(Needle { lower })
    }

    /// Every hit in `text`, which is the line numbered `line`.
    pub fn hits_in(&self, line: LineIndex, text: &str) -> Vec<FindHit> {
        let mut hits = Vec::new();
        let mut at = 0;
        while at < text.len() {
            let Some(rest) = text.get(at..) else { break };
            match self.matched(rest) {
                Some(used) => {
                    hits.push(FindHit {
                        line,
                        from: offset(at),
                        to: offset(at + used),
                    });
                    at += used;
                }
                None => at += rest.chars().next().map_or(1, char::len_utf8),
            }
        }
        hits
    }

    /// How many bytes of `rest` the phrase covers when it starts there, or `None` when it does
    /// not. A phrase must end between two characters: lower-casing one character can give
    /// several, and a phrase that stops inside them is not a match.
    fn matched(&self, rest: &str) -> Option<usize> {
        let mut want = self.lower.iter();
        let mut used = 0;
        for c in rest.chars() {
            for lower in c.to_lowercase() {
                if want.next() != Some(&lower) {
                    return None;
                }
            }
            used += c.len_utf8();
            if want.len() == 0 {
                return Some(used);
            }
        }
        None
    }
}

fn offset(bytes: usize) -> ByteOffset {
    ByteOffset(u32::try_from(bytes).unwrap_or(u32::MAX))
}
