//! Asking a plugin where bayonet cannot start one (Windows: its runner is Unix's). The runner
//! has the requests of the Unix one and each answers that the plugin could not be started, so
//! the viewer shows what it has without the plugin.

use crate::error::PlatformError;
use crate::thumbnail::ThumbPixels;
use anyview_core::work::Stop;
use anyview_core::{FilePath, PixelArea, PixelLen};
use anyview_plugin::Installed;
use anyview_plugin_protocol::{Done, ExportRequest, FactRow, Hello, Progress};
use std::time::Duration;

/// How long the host waits on a plugin. Start from [`Timeouts::default`] and set what differs
/// with the `with_` methods; fields may be added without breaking a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Timeouts {
    /// From starting the plugin to its greeting.
    pub hello: Duration,
    /// The longest a plugin may say nothing while a request is open.
    pub silence: Duration,
    /// After the cancel request, how long the plugin has to stop before it is killed.
    pub cancel_grace: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            hello: Duration::from_secs(5),
            silence: Duration::from_secs(30),
            cancel_grace: Duration::from_secs(2),
        }
    }
}

impl Timeouts {
    /// The same with `hello` as the wait for the greeting.
    pub fn with_hello(self, hello: Duration) -> Timeouts {
        Timeouts { hello, ..self }
    }

    /// The same with `silence` as the longest a plugin may say nothing.
    pub fn with_silence(self, silence: Duration) -> Timeouts {
        Timeouts { silence, ..self }
    }

    /// The same with `cancel_grace` as the time a plugin has to stop after a cancel.
    pub fn with_cancel_grace(self, cancel_grace: Duration) -> Timeouts {
        Timeouts {
            cancel_grace,
            ..self
        }
    }
}

/// Runs requests against plugins. Here none can be started.
#[derive(Debug, Clone, Default)]
pub struct PluginRunner {
    _timeouts: Timeouts,
}

impl PluginRunner {
    /// A runner that waits as `timeouts` say.
    #[must_use]
    pub fn new(timeouts: Timeouts) -> PluginRunner {
        PluginRunner {
            _timeouts: timeouts,
        }
    }

    /// What `plugin` says of itself when started: it cannot be started here.
    pub fn hello(&self, plugin: &Installed) -> Result<Hello, PlatformError> {
        Err(self.cannot_start(plugin))
    }

    /// The rows of facts `plugin` reads from `path`.
    pub fn probe(
        &self,
        plugin: &Installed,
        _path: &FilePath,
    ) -> Result<Vec<FactRow>, PlatformError> {
        Err(self.cannot_start(plugin))
    }

    /// A picture of `path` no larger than `max_edge` on its longer side.
    pub fn thumbnail(
        &self,
        plugin: &Installed,
        _path: &FilePath,
        _max_edge: PixelLen,
    ) -> Result<ThumbPixels, PlatformError> {
        Err(self.cannot_start(plugin))
    }

    /// A picture of `path` with at most `max_area` pixels.
    pub fn decode(
        &self,
        plugin: &Installed,
        _path: &FilePath,
        _max_area: PixelArea,
    ) -> Result<ThumbPixels, PlatformError> {
        Err(self.cannot_start(plugin))
    }

    /// Runs an export.
    pub fn export(
        &self,
        plugin: &Installed,
        _request: &ExportRequest,
        _stop: &Stop,
        _on_progress: impl FnMut(Progress),
    ) -> Result<Done, PlatformError> {
        Err(self.cannot_start(plugin))
    }

    fn cannot_start(&self, plugin: &Installed) -> PlatformError {
        PlatformError::Spawn {
            program: plugin.manifest.id.as_str().to_owned(),
            kind: std::io::ErrorKind::Unsupported,
        }
    }
}
