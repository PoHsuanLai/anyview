//! The one error `anyview-plugin` returns: why a manifest is not a plugin the viewer can use.

use anyview_plugin_protocol::Capability;
use std::path::PathBuf;

/// Why a manifest, or the program it names, cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// The file cannot be read.
    #[error("cannot read the manifest: {kind}")]
    Unreadable {
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// The text is not TOML of the manifest's shape, or names a kind, MIME type or capability
    /// that does not exist.
    #[error("not a manifest: {reason}")]
    Syntax {
        /// The TOML parser's description.
        reason: String,
    },
    /// The id is empty, too long, or has a character outside `a-z 0-9 - _`.
    #[error("not a plugin id: {id:?}")]
    IdInvalid {
        /// The id as written.
        id: String,
    },
    /// The file is not named `<id>.toml`.
    #[error("the manifest names the id {id:?} but the file is called {file:?}")]
    IdFileMismatch {
        /// The id inside.
        id: String,
        /// The file's stem.
        file: String,
    },
    /// The name is empty.
    #[error("the manifest has no name")]
    NameEmpty,
    /// The protocol version is zero.
    #[error("protocol versions start at 1")]
    ProtocolZero,
    /// The manifest provides nothing.
    #[error("the manifest provides no capability")]
    NothingProvided,
    /// A capability is listed twice.
    #[error("the capability {capability:?} is listed twice")]
    CapabilityRepeated {
        /// The repeated one.
        capability: Capability,
    },
    /// A capability handles no kind and no MIME type.
    #[error("the capability {capability:?} handles no kind and no MIME type")]
    HandlesNothing {
        /// The empty one.
        capability: Capability,
    },
    /// An `export` lists no targets.
    #[error("the export capability lists no targets")]
    TargetsMissing,
    /// A target name is empty or has a character outside `a-z 0-9 - _ .`.
    #[error("not a target name: {target:?}")]
    TargetInvalid {
        /// The name as written.
        target: String,
    },
    /// Targets on a capability that is not `export`.
    #[error("only the export capability has targets, not {capability:?}")]
    TargetsMisplaced {
        /// Where they were written.
        capability: Capability,
    },
    /// A capability that is spoken over the protocol, and no `program` to speak it.
    #[error("the capability {capability:?} needs a program")]
    ProgramMissing {
        /// What needs it.
        capability: Capability,
    },
    /// A path that must be absolute is not.
    #[error("the path {path:?} must be absolute")]
    PathNotAbsolute {
        /// The path as written.
        path: PathBuf,
    },
    /// `play` without the player's `mpv` and `cplugin` paths.
    #[error("the play capability needs both `mpv` and `cplugin`")]
    PlayerIncomplete,
    /// `mpv` or `cplugin` on a capability that is not `play`.
    #[error("only the play capability has player paths, not {capability:?}")]
    PlayerMisplaced {
        /// Where they were written.
        capability: Capability,
    },
    /// The plugin speaks a protocol newer than the viewer.
    #[error("the plugin speaks protocol {protocol}, this viewer speaks up to {supported}")]
    ProtocolUnsupported {
        /// The plugin's.
        protocol: u32,
        /// The viewer's newest.
        supported: u32,
    },
    /// A program the manifest names is not there.
    #[error("the program {path:?} does not exist")]
    FileMissing {
        /// The path as written.
        path: PathBuf,
    },
    /// A program the manifest names cannot be run.
    #[error("the program {path:?} is not an executable file")]
    NotExecutable {
        /// The path as written.
        path: PathBuf,
    },
}
