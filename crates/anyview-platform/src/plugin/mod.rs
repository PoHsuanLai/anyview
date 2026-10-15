//! Plugins at the edge: finding their manifests on disk, checking their programs, and talking to
//! them over the protocol. The pure half (the capabilities, the registry) is `anyview-plugin`; the
//! messages are `anyview-plugin-protocol`; the mechanics of finding, starting and containing a
//! plugin are bayonet's.
//!
//! A plugin is started for each request and killed when its call returns or is dropped, so a
//! crash, a hang or a protocol error is a `PlatformError` and never reaches the viewer.

mod discover;
mod route;
// bayonet starts a plugin with a process group, a poll on its pipes and a kill of the group, which
// are Unix's. Elsewhere `runner_absent` has the same runner and every request says that plugins
// cannot be started there.
#[cfg_attr(not(unix), path = "runner_absent.rs")]
mod runner;
#[cfg(unix)]
mod wire;

pub use discover::{Discovery, Rejected, discover, discover_in};
pub use route::PluginFacts;
pub use runner::{PluginRunner, Timeouts};
