//! What a panic leaves behind for a bug report: a short text on stderr and the same text in a file
//! under the state directory. The hook is separate from the job seam's `catch_unwind`: it runs at
//! the moment of the panic, whoever catches it afterwards.

mod files;
mod hook;
mod report;

#[cfg(test)]
mod tests;

pub use files::{CRASH_FOLDER, KEEP, crash_dir, write_report};
pub use hook::install;
pub use report::Report;
