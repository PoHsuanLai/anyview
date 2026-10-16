//! A find's hits as the text view draws them: the list a worker made, which of them are on a
//! line, how a highlighted line is cut where a hit starts and ends, and where the view scrolls
//! to show a hit. The stage machine holds only the count and the current index (`FindHits`);
//! this is the edge's side, the places themselves.

mod found;
mod marks;
mod snippet;

pub use found::FoundHits;
pub(super) use found::hit_place;
pub(crate) use found::top_for;
pub(crate) use marks::{Mark, cut, pieces};
pub(crate) use snippet::{Snippet, clip};

#[cfg(test)]
mod tests;
