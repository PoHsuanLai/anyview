//! What a caller asks the pane to draw: the picture (or page, lines, table), the file's name, the
//! rows of facts.

use ds::prelude::Word;

/// One of the pieces the pane is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Part {
    /// The media box: the picture, page, lines, table or glyph the body stands for.
    Media,
    /// The file's name under it.
    Title,
    /// The rows of facts.
    Facts,
}

/// The parts of the pane a caller wants drawn, a set of [`Part`]. The default is all of them, which
/// is the pane as it was before callers could choose: a mail attachment strip asks for
/// `Parts::of([Part::Media])`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Parts(u8);

impl Parts {
    /// Every part: the whole pane.
    pub const ALL: Parts = Parts(0b111);

    /// Just `parts`.
    #[must_use]
    pub fn of(parts: impl IntoIterator<Item = Part>) -> Self {
        Parts(parts.into_iter().fold(0, |bits, part| bits | bit(part)))
    }

    /// Whether `part` is drawn.
    pub fn contains(self, part: Part) -> bool {
        self.0 & bit(part) != 0
    }
}

impl Default for Parts {
    fn default() -> Self {
        Parts::ALL
    }
}

/// The bit that stands for `part` in a set.
fn bit(part: Part) -> u8 {
    match part {
        Part::Media => 0b001,
        Part::Title => 0b010,
        Part::Facts => 0b100,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_holds_the_parts_it_was_made_of_and_the_default_holds_all() {
        // name, set, parts it holds
        let cases: [(&str, Parts, &[Part]); 4] = [
            ("all", Parts::ALL, &[Part::Media, Part::Title, Part::Facts]),
            (
                "default",
                Parts::default(),
                &[Part::Media, Part::Title, Part::Facts],
            ),
            ("one", Parts::of([Part::Media]), &[Part::Media]),
            ("none", Parts::of([]), &[]),
        ];
        for (name, set, holds) in cases {
            for part in Part::ALL {
                assert_eq!(
                    set.contains(*part),
                    holds.contains(part),
                    "{name}: {part:?}"
                );
            }
        }
        assert_eq!(Parts::of(Part::ALL.iter().copied()), Parts::ALL);
    }
}
