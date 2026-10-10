//! Asking a plugin: the four requests of protocol v1 as blocking calls, each with its timeouts.
//! Starting the plugin, the greeting, the deadlines and the kill on every path out are bayonet's;
//! this says what each request is and what its answer must look like.

use super::wire::Wire;
use crate::error::PlatformError;
use crate::thumbnail::ThumbPixels;
use anyview_core::work::{Stop, StopState};
use anyview_core::{FilePath, PixelArea, PixelLen, PixelSize};
use anyview_plugin::{Installed, Provision};
use anyview_plugin_protocol::{
    Capability, DecodeRequest, Done, ErrorCode, ExportRequest, FactRow, Failure, Frame, Hello,
    HostMessage, ImageHeader, PluginMessage, ProbeRequest, Progress, ThumbnailRequest,
};
use bayonet::run::{RunError, Runner, Session};
use std::ops::ControlFlow;

pub use bayonet::run::Timeouts;

/// Runs requests against plugins. A request starts the plugin, greets it, asks once and kills
/// the process on every path out.
#[derive(Debug, Clone)]
pub struct PluginRunner {
    runner: Runner,
}

impl Default for PluginRunner {
    fn default() -> Self {
        PluginRunner::new(Timeouts::default())
    }
}

impl PluginRunner {
    /// A runner that waits as `timeouts` say.
    #[must_use]
    pub fn new(timeouts: Timeouts) -> PluginRunner {
        PluginRunner {
            runner: Runner::new("anyview", timeouts),
        }
    }

    /// What `plugin` says of itself when started: the requests it answers on this machine and
    /// the export targets it can write. The manifest names every request and target the plugin
    /// knows; a distribution's FFmpeg may encode fewer, and one that is missing answers none.
    /// Blocking: it starts the plugin and kills it.
    pub fn hello(&self, plugin: &Installed) -> Result<Hello, PlatformError> {
        let frame = self.runner.hello::<Wire, Provision>(plugin)?;
        match frame.message {
            PluginMessage::Hello(hello) => Ok(hello),
            other @ (PluginMessage::Facts(_)
            | PluginMessage::Image(_)
            | PluginMessage::Progress(_)
            | PluginMessage::Done(_)
            | PluginMessage::Error(_)) => {
                Err(RunError::unexpected(plugin.manifest.id.as_str(), &other).into())
            }
        }
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
        let mut session = self.open(plugin, Capability::Probe, &request, 0)?;
        match reply(&mut session)? {
            Frame {
                message: PluginMessage::Facts(reply),
                ..
            } => Ok(reply.rows),
            other => Err(unexpected(&session, &other)),
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
        let mut session = self.open(
            plugin,
            Capability::Thumbnail,
            &request,
            picture_bytes(u64::from(max_edge.0).pow(2)),
        )?;
        let (size, pixels) = picture(&mut session)?;
        if size.width.0.max(size.height.0) > max_edge.0 {
            return Err(session
                .broke(format!(
                    "a {} x {} picture exceeds the edge of {}",
                    size.width.0, size.height.0, max_edge.0
                ))
                .into());
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
        let mut session = self.open(
            plugin,
            Capability::Decode,
            &request,
            picture_bytes(max_area.0),
        )?;
        let (size, pixels) = picture(&mut session)?;
        if size.area() > max_area {
            return Err(session
                .broke(format!(
                    "a picture of {} pixels exceeds the budget of {}",
                    size.area().0,
                    max_area.0
                ))
                .into());
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
        let mut session = self.open(plugin, Capability::Export, &message, 0)?;
        session.stream(
            |now| stop.stopped_at(now) == StopState::Stopped,
            &HostMessage::Cancel,
            |id, frame| match frame.message {
                PluginMessage::Progress(progress) => {
                    on_progress(progress);
                    Ok(ControlFlow::Continue(()))
                }
                PluginMessage::Done(done) => Ok(ControlFlow::Break(done)),
                PluginMessage::Error(failure) => Err(failed(id, failure)),
                PluginMessage::Hello(_) | PluginMessage::Facts(_) | PluginMessage::Image(_) => {
                    Err(RunError::unexpected(id, &frame.message).into())
                }
            },
        )
    }

    /// Starts `plugin`, takes its `Hello`, checks it and sends `request`. No reply may carry more
    /// than `payload` bytes after the greeting.
    fn open(
        &self,
        plugin: &Installed,
        capability: Capability,
        request: &HostMessage,
        payload: u64,
    ) -> Result<Session<Wire>, PlatformError> {
        Ok(self
            .runner
            .open::<Wire, Provision>(plugin, capability, request, payload)?)
    }
}

/// The one message that answers a request that has no progress.
fn reply(session: &mut Session<Wire>) -> Result<Frame<PluginMessage>, PlatformError> {
    let frame = session.reply()?;
    match frame.message {
        PluginMessage::Error(failure) => Err(failed(session.id(), failure)),
        PluginMessage::Hello(_)
        | PluginMessage::Facts(_)
        | PluginMessage::Image(_)
        | PluginMessage::Progress(_)
        | PluginMessage::Done(_) => Ok(frame),
    }
}

/// The picture a thumbnail or decode answers with, checked against its header.
fn picture(session: &mut Session<Wire>) -> Result<(PixelSize, ThumbPixels), PlatformError> {
    let frame = reply(session)?;
    let PluginMessage::Image(header) = &frame.message else {
        return Err(unexpected(session, &frame));
    };
    let ImageHeader { width, height } = *header;
    header
        .check(frame.payload.len())
        .map_err(|error| session.broke(error.to_string()))?;
    let size = PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    };
    let pixels = ThumbPixels::new(size, frame.payload)
        .ok_or_else(|| session.broke("the picture is empty"))?;
    Ok((size, pixels))
}

/// The bytes of a picture of at most `area` pixels: RGBA8.
fn picture_bytes(area: u64) -> u64 {
    area.saturating_mul(4)
}

fn failed(plugin: &str, failure: Failure) -> PlatformError {
    match failure.code {
        ErrorCode::Cancelled => PlatformError::PluginCancelled {
            plugin: plugin.to_owned(),
        },
        ErrorCode::Unsupported
        | ErrorCode::Unreadable
        | ErrorCode::Corrupt
        | ErrorCode::TooLarge
        | ErrorCode::Failed => PlatformError::PluginFailed {
            plugin: plugin.to_owned(),
            code: failure.code,
            message: failure.message,
        },
    }
}

fn unexpected(session: &Session<Wire>, frame: &Frame<PluginMessage>) -> PlatformError {
    RunError::unexpected(session.id(), &frame.message).into()
}
