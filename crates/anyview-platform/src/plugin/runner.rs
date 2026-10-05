//! Asking a plugin: the four requests of protocol v1 as blocking calls, each with its timeouts.

use super::process::{Arrival, PluginProcess};
use crate::error::PlatformError;
use crate::thumbnail::ThumbPixels;
use anyview_core::work::{Stop, StopState};
use anyview_core::{FilePath, PixelArea, PixelLen, PixelSize};
use anyview_plugin::Installed;
use anyview_plugin_protocol::{
    Capability, DecodeRequest, Done, ExportRequest, FactRow, Frame, Hello, HostMessage,
    ImageHeader, PluginMessage, ProbeRequest, Progress, ThumbnailRequest,
};
use std::time::{Duration, Instant};

/// How long the host waits on a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// From starting the plugin to its `Hello`.
    pub hello: Duration,
    /// The longest a plugin may say nothing while a request is open; each message starts the
    /// wait again, so an export that reports progress may run as long as it likes.
    pub silence: Duration,
    /// After `Cancel`, how long the plugin has to stop before it is killed.
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

/// How often an export looks at its `Stop` while the plugin is quiet.
const STOP_POLL: Duration = Duration::from_millis(50);

/// Runs requests against plugins. A request starts the plugin, greets it, asks once and kills
/// the process on every path out.
#[derive(Debug, Clone, Copy, Default)]
pub struct PluginRunner {
    timeouts: Timeouts,
}

impl PluginRunner {
    /// A runner that waits as `timeouts` say.
    pub fn new(timeouts: Timeouts) -> PluginRunner {
        PluginRunner { timeouts }
    }

    /// What `plugin` says of itself when started: the requests it answers on this machine and
    /// the export targets it can write. The manifest names every request and target the plugin
    /// knows; a distribution's FFmpeg may encode fewer, and one that is missing answers none.
    /// Blocking: it starts the plugin and kills it.
    pub fn hello(&self, plugin: &Installed) -> Result<Hello, PlatformError> {
        let mut process = PluginProcess::spawn(plugin)?;
        self.greeting(&mut process)
    }

    /// The rows of facts `plugin` reads from `path`. Blocking.
    pub fn probe(
        &self,
        plugin: &Installed,
        path: &FilePath,
    ) -> Result<Vec<FactRow>, PlatformError> {
        let request = HostMessage::Probe(ProbeRequest {
            path: path.as_path().to_path_buf(),
        });
        let mut process = self.open(plugin, Capability::Probe, &request)?;
        match self.reply(&mut process)? {
            Frame {
                message: PluginMessage::Facts(reply),
                ..
            } => Ok(reply.rows),
            other => Err(unexpected(&process, &other)),
        }
    }

    /// A picture of `path` no larger than `max_edge` on its longer side. Blocking.
    pub fn thumbnail(
        &self,
        plugin: &Installed,
        path: &FilePath,
        max_edge: PixelLen,
    ) -> Result<ThumbPixels, PlatformError> {
        let request = HostMessage::Thumbnail(ThumbnailRequest {
            path: path.as_path().to_path_buf(),
            max_edge: max_edge.0,
        });
        let mut process = self.open(plugin, Capability::Thumbnail, &request)?;
        let (size, pixels) = self.picture(&mut process)?;
        if size.width.0.max(size.height.0) > max_edge.0 {
            return Err(process.broke(format!(
                "a {} x {} picture exceeds the edge of {}",
                size.width.0, size.height.0, max_edge.0
            )));
        }
        Ok(pixels)
    }

    /// A picture of `path` with at most `max_area` pixels. Blocking.
    pub fn decode(
        &self,
        plugin: &Installed,
        path: &FilePath,
        max_area: PixelArea,
    ) -> Result<ThumbPixels, PlatformError> {
        let request = HostMessage::Decode(DecodeRequest {
            path: path.as_path().to_path_buf(),
            max_area: max_area.0,
        });
        let mut process = self.open(plugin, Capability::Decode, &request)?;
        let (size, pixels) = self.picture(&mut process)?;
        if size.area() > max_area {
            return Err(process.broke(format!(
                "a picture of {} pixels exceeds the budget of {}",
                size.area().0,
                max_area.0
            )));
        }
        Ok(pixels)
    }

    /// Runs an export, telling `on_progress` about each step. When `stop` is raised the plugin
    /// is asked to cancel and, if it does not stop within the grace period, killed; either way
    /// the result is `PluginCancelled`, unless the plugin finished first. Blocking.
    pub fn export(
        &self,
        plugin: &Installed,
        request: &ExportRequest,
        stop: &Stop,
        mut on_progress: impl FnMut(Progress),
    ) -> Result<Done, PlatformError> {
        let message = HostMessage::Export(request.clone());
        let mut process = self.open(plugin, Capability::Export, &message)?;
        let mut asked_to_cancel: Option<Instant> = None;
        let mut quiet_since = Instant::now();
        loop {
            let now = Instant::now();
            if let Some(since) = asked_to_cancel {
                if now.duration_since(since) >= self.timeouts.cancel_grace {
                    return Err(PlatformError::PluginCancelled {
                        plugin: process.id().to_owned(),
                    });
                }
            } else if stop.stopped_at(now) == StopState::Stopped {
                process.send(&HostMessage::Cancel)?;
                asked_to_cancel = Some(now);
            }
            if now.duration_since(quiet_since) >= self.timeouts.silence {
                return Err(PlatformError::PluginSilent {
                    plugin: process.id().to_owned(),
                    waited: self.timeouts.silence,
                });
            }
            match process.receive(STOP_POLL)? {
                Arrival::Quiet => {}
                Arrival::Message(frame) => {
                    quiet_since = Instant::now();
                    match frame.message {
                        PluginMessage::Progress(progress) => on_progress(progress),
                        PluginMessage::Done(done) => return Ok(done),
                        PluginMessage::Error(failure) => return Err(failed(&process, failure)),
                        PluginMessage::Hello(_)
                        | PluginMessage::Facts(_)
                        | PluginMessage::Image(_) => {
                            return Err(unexpected(&process, &frame));
                        }
                    }
                }
            }
        }
    }

    /// Starts `plugin`, takes its `Hello`, checks it and sends `request`.
    fn open(
        &self,
        plugin: &Installed,
        capability: Capability,
        request: &HostMessage,
    ) -> Result<PluginProcess, PlatformError> {
        let mut process = PluginProcess::spawn(plugin)?;
        let hello = self.greeting(&mut process)?;
        if !hello.provides.contains(&capability) {
            return Err(PlatformError::PluginLacks {
                plugin: process.id().to_owned(),
                capability,
            });
        }
        process.send(request)?;
        Ok(process)
    }

    /// The plugin's `Hello`, checked against the protocol this host speaks.
    fn greeting(&self, process: &mut PluginProcess) -> Result<Hello, PlatformError> {
        let hello = match process.receive(self.timeouts.hello)? {
            Arrival::Message(Frame {
                message: PluginMessage::Hello(hello),
                ..
            }) => hello,
            Arrival::Message(other) => return Err(unexpected(process, &other)),
            Arrival::Quiet => {
                return Err(PlatformError::PluginSilent {
                    plugin: process.id().to_owned(),
                    waited: self.timeouts.hello,
                });
            }
        };
        if hello.protocol != PluginProcess::supported() {
            return Err(PlatformError::PluginVersion {
                plugin: process.id().to_owned(),
                offered: hello.protocol,
                supported: PluginProcess::supported(),
            });
        }
        Ok(hello)
    }

    /// The one message that answers a request that has no progress.
    fn reply(&self, process: &mut PluginProcess) -> Result<Frame<PluginMessage>, PlatformError> {
        match process.receive(self.timeouts.silence)? {
            Arrival::Message(frame) => match frame.message {
                PluginMessage::Error(failure) => Err(failed(process, failure)),
                PluginMessage::Hello(_)
                | PluginMessage::Facts(_)
                | PluginMessage::Image(_)
                | PluginMessage::Progress(_)
                | PluginMessage::Done(_) => Ok(frame),
            },
            Arrival::Quiet => Err(PlatformError::PluginSilent {
                plugin: process.id().to_owned(),
                waited: self.timeouts.silence,
            }),
        }
    }

    /// The picture a thumbnail or decode answers with, checked against its header.
    fn picture(
        &self,
        process: &mut PluginProcess,
    ) -> Result<(PixelSize, ThumbPixels), PlatformError> {
        let frame = self.reply(process)?;
        let PluginMessage::Image(header) = &frame.message else {
            return Err(unexpected(process, &frame));
        };
        let ImageHeader { width, height } = *header;
        header
            .check(frame.payload.len())
            .map_err(|error| process.protocol(&error))?;
        let size = PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        };
        let pixels = ThumbPixels::new(size, frame.payload)
            .ok_or_else(|| process.broke("the picture is empty".to_owned()))?;
        Ok((size, pixels))
    }
}

fn failed(process: &PluginProcess, failure: anyview_plugin_protocol::Failure) -> PlatformError {
    match failure.code {
        anyview_plugin_protocol::ErrorCode::Cancelled => PlatformError::PluginCancelled {
            plugin: process.id().to_owned(),
        },
        anyview_plugin_protocol::ErrorCode::Unsupported
        | anyview_plugin_protocol::ErrorCode::Unreadable
        | anyview_plugin_protocol::ErrorCode::Corrupt
        | anyview_plugin_protocol::ErrorCode::TooLarge
        | anyview_plugin_protocol::ErrorCode::Failed => PlatformError::PluginFailed {
            plugin: process.id().to_owned(),
            code: failure.code,
            message: failure.message,
        },
    }
}

fn unexpected(process: &PluginProcess, frame: &Frame<PluginMessage>) -> PlatformError {
    process.broke(format!("did not expect {:?}", frame.message))
}
