//! Pages: an index, a count that cannot be zero, a range valid by construction, and the choice
//! between "every page" and a range.

use crate::error::CoreError;
use std::num::NonZeroU32;

/// A page's place in a document, counting from zero.
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
pub struct PageIndex(pub u32);

/// How many pages a document has: at least one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct PageCount(NonZeroU32);

impl PageCount {
    /// `count` pages, or `None` for zero (a document with no pages has nothing to show).
    pub fn new(count: u32) -> Option<Self> {
        NonZeroU32::new(count).map(PageCount)
    }

    /// The number of pages.
    pub fn get(self) -> u32 {
        self.0.get()
    }

    /// The last page.
    pub fn last(self) -> PageIndex {
        PageIndex(self.0.get() - 1) // a NonZeroU32 is at least 1
    }

    /// `page`, or the last page when it is past the end.
    pub fn clamp(self, page: PageIndex) -> PageIndex {
        page.min(self.last())
    }

    /// Whether `page` exists.
    pub fn contains(self, page: PageIndex) -> bool {
        page <= self.last()
    }
}

/// A run of pages, first to last inclusive, never reversed and never empty. A range does not know
/// the document it will be applied to; [`PageRange::within`] checks that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "PageRangeWire")]
pub struct PageRange {
    first: PageIndex,
    last: PageIndex,
}

/// The stored shape of a [`PageRange`], checked on load.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct PageRangeWire {
    first: PageIndex,
    last: PageIndex,
}

impl TryFrom<PageRangeWire> for PageRange {
    type Error = CoreError;

    fn try_from(wire: PageRangeWire) -> Result<Self, CoreError> {
        PageRange::new(wire.first, wire.last)
    }
}

impl PageRange {
    /// Pages `first` to `last` inclusive, or why the range is reversed.
    pub fn new(first: PageIndex, last: PageIndex) -> Result<Self, CoreError> {
        if last < first {
            return Err(CoreError::PageRangeReversed {
                start: first.0,
                end: last.0,
            });
        }
        Ok(PageRange { first, last })
    }

    /// The range of one page.
    pub fn single(page: PageIndex) -> Self {
        PageRange {
            first: page,
            last: page,
        }
    }

    /// The first page.
    pub fn first(self) -> PageIndex {
        self.first
    }

    /// The last page.
    pub fn last(self) -> PageIndex {
        self.last
    }

    /// How many pages the range covers.
    pub fn len(self) -> PageCount {
        let span = self.last.0 - self.first.0; // last >= first by construction
        PageCount::new(span.saturating_add(1)).unwrap_or(PageCount(NonZeroU32::MIN))
    }

    /// Whether `page` is in the range.
    pub fn contains(self, page: PageIndex) -> bool {
        self.first <= page && page <= self.last
    }

    /// The range cut to a document of `count` pages, or `None` when it starts past the end.
    pub fn within(self, count: PageCount) -> Option<PageRange> {
        count.contains(self.first).then(|| PageRange {
            first: self.first,
            last: count.clamp(self.last),
        })
    }
}

/// Which pages an operation covers. A person picks "all pages" before a document's length is
/// known, so the default choice is not a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageSelection {
    /// Every page.
    #[default]
    All,
    /// A run of pages.
    Range(PageRange),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(first: u32, last: u32) -> PageRange {
        PageRange::new(PageIndex(first), PageIndex(last)).unwrap()
    }

    #[test]
    fn a_page_count_is_never_zero() {
        assert_eq!(PageCount::new(0), None);
        assert_eq!(PageCount::new(3).map(PageCount::get), Some(3));
    }

    #[test]
    fn page_count_clamps_and_contains() {
        const CASES: &[(&str, u32, u32, u32, bool)] = &[
            // name, count, page, clamped, contained
            ("inside", 5, 2, 2, true),
            ("last", 5, 4, 4, true),
            ("past the end", 5, 5, 4, false),
            ("far past", 5, 900, 4, false),
            ("one page", 1, 0, 0, true),
        ];
        for (name, count, page, clamped, contained) in CASES {
            let count = PageCount::new(*count).unwrap();
            assert_eq!(count.clamp(PageIndex(*page)), PageIndex(*clamped), "{name}");
            assert_eq!(count.contains(PageIndex(*page)), *contained, "{name}");
        }
    }

    #[test]
    fn a_reversed_range_is_refused() {
        assert_eq!(
            PageRange::new(PageIndex(4), PageIndex(2)),
            Err(CoreError::PageRangeReversed { start: 4, end: 2 })
        );
        assert!(PageRange::new(PageIndex(2), PageIndex(2)).is_ok());
    }

    #[test]
    fn range_length_and_membership() {
        const CASES: &[(&str, u32, u32, u32, u32, bool)] = &[
            // name, first, last, len, probe, contains(probe)
            ("one page", 3, 3, 1, 3, true),
            ("run", 2, 6, 5, 6, true),
            ("before", 2, 6, 5, 1, false),
            ("after", 2, 6, 5, 7, false),
        ];
        for (name, first, last, len, probe, contains) in CASES {
            let r = range(*first, *last);
            assert_eq!(r.len().get(), *len, "{name}");
            assert_eq!(r.contains(PageIndex(*probe)), *contains, "{name}");
        }
    }

    #[test]
    fn a_range_is_cut_to_the_document() {
        // name, first, last, document pages, result
        type Row = (&'static str, u32, u32, u32, Option<(u32, u32)>);
        const CASES: &[Row] = &[
            ("fits", 1, 3, 10, Some((1, 3))),
            ("tail cut", 8, 20, 10, Some((8, 9))),
            ("starts past the end", 10, 12, 10, None),
        ];
        for (name, first, last, pages, want) in CASES {
            let got = range(*first, *last)
                .within(PageCount::new(*pages).unwrap())
                .map(|r| (r.first().0, r.last().0));
            assert_eq!(got, *want, "{name}");
        }
    }

    #[test]
    fn a_range_round_trips_and_a_reversed_one_is_refused_on_load() {
        let r = range(1, 4);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, r#"{"first":1,"last":4}"#);
        assert_eq!(serde_json::from_str::<PageRange>(&json).unwrap(), r);
        assert!(serde_json::from_str::<PageRange>(r#"{"first":4,"last":1}"#).is_err());
        assert_eq!(serde_json::from_str::<PageCount>("7").unwrap().get(), 7);
        assert!(serde_json::from_str::<PageCount>("0").is_err());
    }
}
