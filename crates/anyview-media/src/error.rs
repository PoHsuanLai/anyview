//! The crate's one error: what a caller of the player or of an export can act on.

use std::path::PathBuf;

/// Why the player, a probe or an export could not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaError {
    /// libmpv refused a call, or the player could not be made.
    #[error("the player failed: {0}")]
    Player(String),
    /// libav refused a call: the file is not a recording it reads, or a codec failed.
    #[error("libav failed: {0}")]
    Libav(String),
    /// No graphics adapter opened, so the player has nothing to draw on.
    #[error("no graphics adapter for the player")]
    NoDevice,
    /// The file named could not be opened or written.
    #[error("cannot {op} {path}: {kind}")]
    Io {
        /// What was being done, as a verb: `read`, `write`, `create`.
        op: &'static str,
        /// The file.
        path: PathBuf,
        /// What the system said.
        kind: std::io::ErrorKind,
    },
    /// An audio export of a recording that has no audio track.
    #[error("the recording has no audio track")]
    NoAudio,
    /// A frame export of a recording that shows no picture.
    #[error("the recording shows no picture")]
    NoPicture,
    /// The system's libav has no encoder for what was asked.
    #[error("this system's libav has no {0} encoder")]
    EncoderMissing(&'static str),
    /// The work was stopped before it finished; what it had written is removed.
    #[error("the export was stopped")]
    Stopped,
    /// The job is not one this crate carries out (a PDF or an image encode).
    #[error("not a media job")]
    NotMedia,
}
