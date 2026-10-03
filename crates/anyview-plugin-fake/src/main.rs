//! A plugin for the tests: it serves protocol v1 for files whose first line is its text, and
//! misbehaves in the one way its command line asks for. Everything it knows about the viewer is
//! the protocol crate, which is what a third-party plugin needs too.

mod behaviour;
mod serve;

use behaviour::Behaviour;
use std::process::ExitCode;

fn main() -> ExitCode {
    let behaviour = Behaviour::from_args(std::env::args().skip(1));
    serve::run(&behaviour)
}
