//! The program: from the command line to the event loop. `main` calls [`run`]; the parts it
//! joins (claiming the instance, relaying the requests that arrive later, building the pool and
//! the desktop) are separate files so each is tested with fakes.

mod relay;
mod role;
mod start;

#[cfg(test)]
mod tests;

pub use relay::{Arrival, Want, open_each, open_windows, relay, wants_of};
pub use role::{Role, claim_role};
pub use start::{WARM_FOR, run};
