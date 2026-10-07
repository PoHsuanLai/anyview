//! Fakes for the traits, for the tests of the crates that consume them. Each fake is cheap to
//! clone and its clones share their record, so a test hands one clone to the code under test and
//! keeps the other to read what happened.

mod apps;
#[cfg(all(feature = "quire-desktop", target_os = "linux"))]
mod bus;
mod instance;
mod link;
mod media;
#[cfg(all(feature = "quire-desktop", target_os = "linux"))]
mod mpris_client;
mod picker;
mod plugins;
mod printer;
mod reveal;
mod share;
mod spawn;
mod stacking;
mod thumbnails;

pub use apps::FakeApps;
#[cfg(all(feature = "quire-desktop", target_os = "linux"))]
pub use bus::PrivateBus;
pub use instance::{FakeInstance, FakeRole};
pub use link::FakeLinks;
pub use media::{FakeMediaHandle, FakeMediaSession};
#[cfg(all(feature = "quire-desktop", target_os = "linux"))]
pub use mpris_client::MprisClient;
pub use picker::FakePicker;
pub use plugins::{PluginSet, plugins_with};
pub use printer::FakePrinter;
pub use reveal::FakeReveal;
pub use share::FakeShare;
pub use spawn::RecordingSpawn;
pub use stacking::{FakeStacking, StackingSupport};
pub use thumbnails::FakeThumbnails;

use std::sync::{Mutex, MutexGuard, PoisonError};

/// The record behind a lock. A test that panicked while holding it left a record that is still
/// good to read.
pub(crate) fn locked<T>(record: &Mutex<T>) -> MutexGuard<'_, T> {
    record.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
