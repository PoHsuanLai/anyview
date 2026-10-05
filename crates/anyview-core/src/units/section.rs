//! Sections of a book: the chapter of an EPUB or the page of a comic, one at a time.

use std::num::NonZeroU32;

/// A section's place in a book's reading order, counting from zero.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct SectionIndex(pub u32);

/// How many sections a book has: at least one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct SectionCount(NonZeroU32);

impl SectionCount {
    /// `count` sections, or `None` for zero (a book with nothing to read has nothing to show).
    pub fn new(count: u32) -> Option<Self> {
        NonZeroU32::new(count).map(SectionCount)
    }

    /// The number of sections.
    pub fn get(self) -> u32 {
        self.0.get()
    }

    /// The last section.
    pub fn last(self) -> SectionIndex {
        SectionIndex(self.0.get() - 1) // a NonZeroU32 is at least 1
    }

    /// `section`, or the last section when it is past the end.
    pub fn clamp(self, section: SectionIndex) -> SectionIndex {
        section.min(self.last())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_past_the_end_is_the_last_and_zero_sections_is_no_count() {
        let count = SectionCount::new(3).unwrap();
        assert_eq!(count.clamp(SectionIndex(1)), SectionIndex(1));
        assert_eq!(count.clamp(SectionIndex(9)), SectionIndex(2));
        assert_eq!(SectionCount::new(0), None);
    }
}
