//! The one error `anyview-platform` returns, with variants a caller acts on: no bus means the
//! feature is off, a failed call is reported, a bad file is skipped.

use anyview_plugin_protocol::{Capability, ErrorCode};
use std::path::PathBuf;
use std::time::Duration;

/// What the edge was doing to a file or directory when the disk refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOp {
    /// Reading a file.
    Read,
    /// Creating a directory.
    CreateDir,
    /// Writing a file.
    Write,
    /// Renaming a file over another.
    Rename,
}

/// Why a platform call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// The environment names no session bus, so nothing that needs one can run.
    #[error("there is no session bus")]
    NoBus,
    /// A D-Bus call, registration or reply failed. A caller reports it and carries on without
    /// the feature.
    #[error("{call} failed on the bus: {reason}")]
    Bus {
        /// What was being done: a method, a name or an object path.
        call: &'static str,
        /// What the bus library reported.
        reason: String,
    },
    /// The disk refused an operation.
    #[error("{op:?} failed on {path:?}: {kind}")]
    Io {
        /// What was being done.
        op: IoOp,
        /// The file or directory.
        path: PathBuf,
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// A thumbnail file or the pixels offered for one cannot be a PNG.
    #[error("{path:?} is not a usable thumbnail: {reason}")]
    Thumbnail {
        /// The thumbnail file, or the file it would be for.
        path: PathBuf,
        /// The PNG codec's description.
        reason: String,
    },
    /// A program could not be started.
    #[error("cannot start {program:?}: {kind}")]
    Spawn {
        /// The program as named.
        program: String,
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// A desktop entry's `Exec` line cannot become a command.
    #[error("the entry {id:?} has no usable Exec line: {reason}")]
    Exec {
        /// The entry's id.
        id: String,
        /// The parser's description.
        reason: String,
    },
    /// A plugin sent nothing for too long and was killed.
    #[error("the plugin {plugin} was silent for {waited:?}")]
    PluginSilent {
        /// The plugin's id.
        plugin: String,
        /// How long the host waited.
        waited: Duration,
    },
    /// A plugin ended without answering: it crashed, was killed, or exited early.
    #[error("the plugin {plugin} ended without an answer ({status})")]
    PluginCrashed {
        /// The plugin's id.
        plugin: String,
        /// How it ended, with the last line it wrote to stderr when it wrote one.
        status: String,
    },
    /// A plugin broke the protocol: bytes that are not a message, a message out of turn, a
    /// picture of the wrong size.
    #[error("the plugin {plugin} broke the protocol: {reason}")]
    PluginProtocol {
        /// The plugin's id.
        plugin: String,
        /// What was wrong.
        reason: String,
    },
    /// A plugin speaks a version of the protocol this viewer does not.
    #[error("the plugin {plugin} speaks protocol {offered}, this viewer speaks {supported}")]
    PluginVersion {
        /// The plugin's id.
        plugin: String,
        /// What it said in its `Hello`.
        offered: u32,
        /// What the viewer speaks.
        supported: u32,
    },
    /// A plugin's `Hello` does not list the capability that was asked of it.
    #[error("the plugin {plugin} does not answer {capability:?}")]
    PluginLacks {
        /// The plugin's id.
        plugin: String,
        /// What was asked.
        capability: Capability,
    },
    /// A plugin answered with an error of its own.
    #[error("the plugin {plugin} failed ({code:?}): {message}")]
    PluginFailed {
        /// The plugin's id.
        plugin: String,
        /// What a caller can act on.
        code: ErrorCode,
        /// What a person can read.
        message: String,
    },
    /// An export was cancelled and the plugin stopped (or was killed after the grace period).
    #[error("the plugin {plugin} was stopped")]
    PluginCancelled {
        /// The plugin's id.
        plugin: String,
    },
}

impl From<bayonet::run::RunError<Capability>> for PlatformError {
    fn from(error: bayonet::run::RunError<Capability>) -> Self {
        use bayonet::run::RunError;
        match error {
            RunError::Spawn { program, kind } => PlatformError::Spawn { program, kind },
            RunError::Silent { plugin, waited } => PlatformError::PluginSilent { plugin, waited },
            RunError::Crashed { plugin, status } => PlatformError::PluginCrashed { plugin, status },
            RunError::Protocol { plugin, reason } => {
                PlatformError::PluginProtocol { plugin, reason }
            }
            RunError::Version {
                plugin,
                offered,
                supported,
            } => PlatformError::PluginVersion {
                plugin,
                offered,
                supported,
            },
            RunError::Lacks { plugin, capability } => {
                PlatformError::PluginLacks { plugin, capability }
            }
            RunError::Cancelled { plugin } => PlatformError::PluginCancelled { plugin },
        }
    }
}

impl PlatformError {
    /// A failed bus call named `call`.
    pub(crate) fn bus(call: &'static str, error: impl std::fmt::Display) -> Self {
        PlatformError::Bus {
            call,
            reason: error.to_string(),
        }
    }

    /// A failed disk operation.
    pub(crate) fn io(op: IoOp, path: impl Into<PathBuf>, error: &std::io::Error) -> Self {
        PlatformError::Io {
            op,
            path: path.into(),
            kind: error.kind(),
        }
    }
}
