//! The one error the picture plugins raise, and the code the host is told.

use anyview_plugin_protocol::{ErrorCode, WireError};

/// Why a request could not be done.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// A program is not where it was looked for.
    #[error("{tool} was not found: {looked}")]
    ToolMissing {
        /// The program's name.
        tool: String,
        /// Where it was looked for.
        looked: String,
    },
    /// The file cannot be read.
    #[error("cannot read the file: {0}")]
    Unreadable(String),
    /// The tool wrote something that is not a picture.
    #[error("the tool's output is not a picture: {0}")]
    Corrupt(String),
    /// The plugin does not do this.
    #[error("{0}")]
    Unsupported(String),
    /// The picture is beyond what the plugin will hold.
    #[error("{0}")]
    TooLarge(String),
    /// The tool ran and failed.
    #[error("{tool} failed: {message}")]
    Failed {
        /// The program's name.
        tool: String,
        /// What it said.
        message: String,
    },
    /// The tool ran past its deadline and was killed.
    #[error("{tool} did not finish in {seconds} s and was stopped")]
    TimedOut {
        /// The program's name.
        tool: String,
        /// The deadline.
        seconds: u64,
    },
    /// The host asked it to stop.
    #[error("cancelled")]
    Cancelled,
    /// A file or a pipe failed.
    #[error("{op}: {kind}")]
    Io {
        /// What was being done.
        op: &'static str,
        /// What the system said.
        kind: std::io::ErrorKind,
    },
    /// The host's pipe failed.
    #[error("the host's pipe failed: {0}")]
    Host(#[from] WireError),
}

impl ToolError {
    /// What the host is told to act on.
    pub fn code(&self) -> ErrorCode {
        match self {
            ToolError::Unreadable(_) => ErrorCode::Unreadable,
            ToolError::Corrupt(_) => ErrorCode::Corrupt,
            ToolError::Unsupported(_) => ErrorCode::Unsupported,
            ToolError::TooLarge(_) => ErrorCode::TooLarge,
            ToolError::Cancelled => ErrorCode::Cancelled,
            ToolError::ToolMissing { .. }
            | ToolError::Failed { .. }
            | ToolError::TimedOut { .. }
            | ToolError::Io { .. }
            | ToolError::Host(_) => ErrorCode::Failed,
        }
    }

    /// An I/O failure while doing `op`.
    pub fn io(op: &'static str, error: &std::io::Error) -> ToolError {
        ToolError::Io {
            op,
            kind: error.kind(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_failure_has_the_code_a_host_acts_on() {
        let cases: Vec<(&str, ToolError, ErrorCode)> = vec![
            (
                "unreadable",
                ToolError::Unreadable("x".into()),
                ErrorCode::Unreadable,
            ),
            (
                "corrupt",
                ToolError::Corrupt("x".into()),
                ErrorCode::Corrupt,
            ),
            (
                "unsupported",
                ToolError::Unsupported("x".into()),
                ErrorCode::Unsupported,
            ),
            (
                "too large",
                ToolError::TooLarge("x".into()),
                ErrorCode::TooLarge,
            ),
            ("cancelled", ToolError::Cancelled, ErrorCode::Cancelled),
            (
                "timed out",
                ToolError::TimedOut {
                    tool: "t".into(),
                    seconds: 1,
                },
                ErrorCode::Failed,
            ),
        ];
        for (name, error, code) in cases {
            assert_eq!(error.code(), code, "{name}");
        }
    }
}
