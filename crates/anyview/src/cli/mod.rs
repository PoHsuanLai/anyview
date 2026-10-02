//! The command line, parsed once at the boundary into what a launch asks of the viewer. Nothing
//! below this reads an argument.

mod parse;

#[cfg(test)]
mod tests;

pub use parse::{CliError, Invocation, USAGE, parse};
