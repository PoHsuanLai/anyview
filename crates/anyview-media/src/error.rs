//! The crate's one error: what a caller of the player or of an export can act on.

use std::path::PathBuf;

/// Why the player, a probe or an export could not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaError {
    /// mpv refused a call, or the player could not be made.
    #[error("the player failed: {0}")]
    Player(String),
    /// No plugin plays recordings: this package would.
    #[error("no plugin plays recordings; {0} would")]
    PlayerMissing(&'static str),
    /// The mpv process exited or crashed; a new session is the only way to play again.
    #[error("the mpv process stopped")]
    PlayerGone,
    /// The mpv process did not answer in time; it may be hung.
    #[error("the mpv process did not answer")]
    PlayerSilent,
    /// mpv or its C plugin could not be started: not found, the wrong version, or refused.
    #[error("cannot start mpv: {0}")]
    PlayerStart(String),
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
