//! What the window itself holds, as opposed to what a machine does: where a probe stands, the
//! document the load opened, the window of lines last read. These are the results of effects,
//! kept so the views can draw them and so a late result for a file the person left is dropped.
//! The commands, capsule slots and palette rows, and the parameters the machines read, are
//! worked out from them in the modules below.

mod commands;
mod offers;
mod params;
mod probe;
mod rows;

pub(super) use offers::{MarkupOffer, PictureOffer};
pub(super) use params::{Live, params};
pub(super) use probe::{Probe, family};
pub(super) use rows::{BRIEF_HITS, WHOLE_HITS, offered_slots, without_find};

#[cfg(test)]
mod tests;
