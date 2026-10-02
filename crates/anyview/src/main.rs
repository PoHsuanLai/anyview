//! The viewer binary. Launch, single instance and the command line arrive with the platform
//! wiring; until a file can be named the program says how it is used and fails.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("usage: anyview <file>...");
    ExitCode::FAILURE
}
