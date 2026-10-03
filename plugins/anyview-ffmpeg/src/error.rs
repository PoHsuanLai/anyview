//! The one error this plugin raises, and the code the host is told.

use anyview_plugin_protocol::{ErrorCode, ProtocolError};
use std::path::PathBuf;

/// Why a request could not be done.
#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    /// A program is not where it was looked for.
    #[error("{tool} was not found: {looked}")]
    ToolMissing {
        /// `ffmpeg` or `ffprobe`.
        tool: &'static str,
        /// Where it was looked for.
        looked: String,
    },
    /// A program is there and does not answer as FFmpeg does.
    #[error("{tool} cannot be used: {reason}")]
    ToolBroken {
        /// `ffmpeg` or `ffprobe`.
        tool: &'static str,
        /// What went wrong.
        reason: String,
    },
    /// The file cannot be read.
    #[error("cannot read the file: {0}")]
    Unreadable(String),
    /// The file is damaged or not a recording.
    #[error("not a readable recording: {0}")]
    Corrupt(String),
    /// The file or the target is beyond what is asked.
    #[error("{0}")]
    Unsupported(String),
    /// The output is already there; an export never replaces a file.
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    /// ffmpeg ran and failed.
    #[error("ffmpeg failed: {0}")]
    Failed(String),
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
    Host(#[from] ProtocolError),
}

impl FfmpegError {
    /// What the host is told to act on.
    pub fn code(&self) -> ErrorCode {
        match self {
            FfmpegError::Unreadable(_) => ErrorCode::Unreadable,
            FfmpegError::Corrupt(_) => ErrorCode::Corrupt,
            FfmpegError::Unsupported(_) => ErrorCode::Unsupported,
            FfmpegError::Cancelled => ErrorCode::Cancelled,
            FfmpegError::ToolMissing { .. }
            | FfmpegError::ToolBroken { .. }
            | FfmpegError::Exists(_)
            | FfmpegError::Failed(_)
            | FfmpegError::Io { .. }
            | FfmpegError::Host(_) => ErrorCode::Failed,
        }
    }

    /// An I/O failure while doing `op`.
    pub fn io(op: &'static str, error: &std::io::Error) -> FfmpegError {
        FfmpegError::Io {
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
        let cases: Vec<(&str, FfmpegError, ErrorCode)> = vec![
            (
                "unreadable",
                FfmpegError::Unreadable("x".into()),
                ErrorCode::Unreadable,
            ),
            (
                "corrupt",
                FfmpegError::Corrupt("x".into()),
                ErrorCode::Corrupt,
            ),
            (
                "unsupported",
                FfmpegError::Unsupported("x".into()),
                ErrorCode::Unsupported,
            ),
            ("cancelled", FfmpegError::Cancelled, ErrorCode::Cancelled),
            (
                "exists",
                FfmpegError::Exists("/a".into()),
                ErrorCode::Failed,
            ),
            ("failed", FfmpegError::Failed("x".into()), ErrorCode::Failed),
        ];
        for (name, error, code) in cases {
            assert_eq!(error.code(), code, "{name}");
        }
    }
}
