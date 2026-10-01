//! Finding, shared by the PDF and text stages: the hits themselves live with whoever searched
//! (a document can have thousands), so a stage holds only which one is current and how many
//! there are, and asks for the others to be shown.

use crate::typed::TypedText;
use std::num::NonZeroU32;

/// How many hits a search found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct HitCount(pub u32);

/// Which hit, from 0 in document order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct HitIndex(pub u32);

/// Which way to step through the hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitStep {
    /// Towards the end of the document, wrapping to the first.
    Next,
    /// Towards the start, wrapping to the last.
    Previous,
}

/// Where a search stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindHits {
    /// The search has been asked for and has not answered.
    Pending,
    /// It answered with nothing.
    NoMatch,
    /// It found hits, and one is current.
    Found(HitCursor),
}

/// The current hit among a non-zero number of hits. It cannot point past the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitCursor {
    current: u32,
    count: NonZeroU32,
}

impl FindHits {
    /// What a search's answer means: `count` hits, the one nearest where the person is first.
    pub const fn answered(count: HitCount, nearest: HitIndex) -> FindHits {
        match NonZeroU32::new(count.0) {
            Some(count) => FindHits::Found(HitCursor {
                current: nearest.0 % count.get(),
                count,
            }),
            None => FindHits::NoMatch,
        }
    }

    /// The hits after stepping; `Pending` and `NoMatch` have nowhere to go and stay as they
    /// are.
    pub fn stepped(self, step: HitStep) -> FindHits {
        match self {
            FindHits::Found(cursor) => FindHits::Found(cursor.stepped(step)),
            FindHits::Pending | FindHits::NoMatch => self,
        }
    }

    /// The current hit, when there is one.
    pub fn current(self) -> Option<HitIndex> {
        match self {
            FindHits::Found(cursor) => Some(HitIndex(cursor.current)),
            FindHits::Pending | FindHits::NoMatch => None,
        }
    }
}

impl HitCursor {
    fn stepped(self, step: HitStep) -> HitCursor {
        let count = self.count.get();
        let current = match step {
            HitStep::Next => (self.current + 1) % count,
            HitStep::Previous => (self.current + count - 1) % count,
        };
        HitCursor { current, ..self }
    }
}

/// What a find wants done, for either stage that finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindOut {
    /// Search the document for this text and answer with the hit count.
    Search(TypedText),
    /// Show this hit: scroll to it and mark it.
    ShowHit(HitIndex),
    /// Take the hit marks away.
    Clear,
}
