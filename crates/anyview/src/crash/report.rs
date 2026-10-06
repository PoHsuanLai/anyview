//! The text of one crash report.

use std::fmt;

/// What a panic knew about itself, and the build that panicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The panicking thread's name, or `<unnamed>`.
    pub thread: String,
    /// The panic's message.
    pub message: String,
    /// `file:line:column` of the panic, when known.
    pub location: String,
    /// The program's version.
    pub version: String,
    /// When it happened, in seconds since the epoch.
    pub at: u64,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "anyview {} crashed", self.version)?;
        writeln!(f, "thread:   {}", self.thread)?;
        writeln!(f, "message:  {}", self.message)?;
        writeln!(f, "location: {}", self.location)?;
        writeln!(f, "time:     {} (seconds since the epoch)", self.at)
    }
}
