//! Plugins as values (ARCHITECTURE section 2l). Codecs and every other piece of code that
//! is copyleft or patent-encumbered run as separate programs the person installs; this crate is
//! the pure half of knowing about them: the manifest a plugin installs (`Manifest`, bayonet's
//! envelope around the viewer's `Provision`s), the registry that says which plugin serves a kind
//! and capability (`Plugins`), and the package to suggest when none does (`MissingPlugin`).
//! The envelope, the ranking and the package type are bayonet's, generic over the capability;
//! the capabilities and what each handles are this crate's. Finding the manifests on disk and
//! talking to the programs belong to `anyview-platform`.
//!
//! Every public item is reached from this root, once.

mod error;
mod handles;
mod manifest;
mod missing;
mod provision;
mod registry;

pub use error::{PluginError, ProvisionFault};
pub use handles::{Handles, Subject};
pub use manifest::{Manifest, PathRole, PluginId, Program};
pub use missing::{MissingPlugin, Package, suggested_package};
pub use provision::{ExportProvision, PlayProvision, Player, Provision, TargetName};
pub use registry::{Candidate, Installed, Origin, Plugins, Readiness, Route, Unusable};
