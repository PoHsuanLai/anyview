//! Plugins at the edge: finding their manifests on disk, checking their programs, and talking to
//! them over the protocol. The pure half (the manifest, the registry) is `anyview-plugin`; the
//! messages are `anyview-plugin-protocol`.
//!
//! A plugin is started for each request and killed when its call returns or is dropped, so a
//! crash, a hang or a protocol error is a `PlatformError` and never reaches the viewer.

mod discover;
mod process;
mod route;
mod runner;

pub use discover::{Discovery, Rejected, discover};
pub use route::PluginFacts;
pub use runner::{PluginRunner, Timeouts};
