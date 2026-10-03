//! Plugins as values (ARCHITECTURE section 2l). Codecs and every other piece of code that
//! is copyleft or patent-encumbered run as separate programs the person installs; this crate is
//! the pure half of knowing about them: the manifest a plugin installs (`Manifest`), its
//! validation, the registry that says which plugin serves a kind and capability (`Plugins`), and
//! the package to suggest when none does (`MissingPlugin`). Finding the manifests on disk and
//! talking to the programs belong to `anyview-platform`.
//!
//! Every public item is reached from this root, once.

mod error;
mod handles;
mod manifest;
mod missing;
mod provision;
mod registry;

pub use error::PluginError;
pub use handles::{Handles, Subject};
pub use manifest::{Manifest, PluginId, Program};
pub use missing::{MissingPlugin, Package, suggested_package};
pub use provision::{ExportProvision, PlayProvision, Player, Provision, TargetName};
pub use registry::{Candidate, Installed, Origin, Plugins, Readiness, Route, Unusable};
