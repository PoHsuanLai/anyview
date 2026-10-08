//! Why a manifest, or the program it names, is not a plugin the viewer can use.

use anyview_plugin_protocol::Capability;

/// Why a manifest, or the program it names, cannot be used: bayonet's shared reasons (the file,
/// the id, the protocol, the programs) with this viewer's own in [`ProvisionFault`].
pub type PluginError = bayonet::manifest::ManifestError<Capability, ProvisionFault>;

/// Why the viewer refuses a `[[provides]]` entry that is well formed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProvisionFault {
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
    /// `play` without the player's `mpv` and `cplugin` paths.
    #[error("the play capability needs both `mpv` and `cplugin`")]
    PlayerIncomplete,
    /// `mpv` or `cplugin` on a capability that is not `play`.
    #[error("only the play capability has player paths, not {capability:?}")]
    PlayerMisplaced {
        /// Where they were written.
        capability: Capability,
    },
}
