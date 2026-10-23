//! Opening Settings at a place: a page, a key, or one Extension's row.

use crate::error::PlatformError;
use ds_core::settings::SettingsTarget;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

/// The request to open Settings, ending when it has been made.
pub type Opening<'a> = Pin<Box<dyn Future<Output = Result<(), PlatformError>> + Send + 'a>>;

/// Open the desktop's Settings at a [`SettingsTarget`], as a button the person pressed asks (the
/// Turn On… of a plugin that is turned off). Object safe, so a host holds one without knowing which
/// platform it runs on.
pub trait OpenSettings: Debug + Send + Sync + 'static {
    /// Show Settings at `target`.
    fn open<'a>(&'a self, target: &'a SettingsTarget) -> Opening<'a>;

    /// Whether this implementation has a Settings behind it. One that answers "not available" to
    /// every request says `false`, so the views never offer what it cannot do.
    fn present(&self) -> bool {
        true
    }
}
