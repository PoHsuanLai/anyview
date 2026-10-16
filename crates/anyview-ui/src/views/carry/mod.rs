//! What the window does with each output of the root machine. Nothing here decides: an output is
//! carried out as a job, a request to the host, or a change to what the window holds.

mod file;
mod out;
mod stage;

pub(super) use out::{Carry, carry_out};

#[cfg(test)]
mod tests;
