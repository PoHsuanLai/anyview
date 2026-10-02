//! Loading a file: sniff it, show the cheap first frame, then swap in the full open. Results of
//! a file the person already left are ignored by ticket.

mod fresh;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use fresh::{Freshness, freshness};
pub use model::{Load, LoadFailure, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket};
