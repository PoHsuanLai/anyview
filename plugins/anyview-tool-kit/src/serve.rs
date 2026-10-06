//! The plugin's main loop: greet with what this machine can do, take the one request, answer it.

use crate::error::ToolError;
use crate::picture::{Fit, Picture};
use crate::run::Stop;
use anyview_plugin_protocol::{
    Capability, Failure, Hello, HostMessage, ImageHeader, PROTOCOL_VERSION, PluginMessage,
    read_frame, write_frame,
};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

/// What a picture plugin does. The loop owns the protocol; the backend owns the tools.
pub trait Backend {
    /// The plugin's name, for logs and the greeting.
    fn name(&self) -> &'static str;

    /// What this machine can do: empty when the tools are not there (the log says why).
    fn provides(&self) -> Vec<Capability>;

    /// The picture of the file at `path` with at most `max_area` pixels, upright. Runs tools and
    /// ends them when `stop` is raised.
    fn decode(&self, path: &Path, max_area: u64, stop: &Stop) -> Result<Picture, ToolError>;

    /// A small picture of the file at `path`, no side longer than `max_edge`.
    fn thumbnail(&self, path: &Path, max_edge: u32, stop: &Stop) -> Result<Picture, ToolError>;
}

/// Serves the one request that arrives.
pub fn serve(backend: &impl Backend) -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("anyview-{} {}", backend.name(), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let mut out = std::io::stdout();
    let provides = backend.provides();
    if provides.is_empty() {
        eprintln!("anyview-{}: offering nothing", backend.name());
    }
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        name: backend.name().to_owned(),
        provides: provides.clone(),
        targets: Vec::new(),
    };
    if write_frame(&mut out, &PluginMessage::Hello(hello), &[]).is_err() {
        return ExitCode::FAILURE;
    }
    if provides.is_empty() {
        return ExitCode::SUCCESS;
    }
    let mut input = std::io::stdin().lock();
    let Ok(frame) = read_frame::<_, HostMessage>(&mut input) else {
        return ExitCode::SUCCESS;
    };
    drop(input);
    // The host may send `Cancel`, or close its end, while a tool runs: a thread of its own reads
    // the pipe and raises the stop, so the tool is killed at once.
    let stop = Stop::new();
    let watcher = stop.clone();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        while let Ok(frame) = read_frame::<_, HostMessage>(&mut input) {
            if frame.message == HostMessage::Cancel {
                break;
            }
        }
        watcher.raise();
    });
    let answered = answer(backend, &provides, frame.message, &stop, &mut out);
    let sent = match answered {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("anyview-{}: {error}", backend.name());
            write_frame(
                &mut out,
                &PluginMessage::Error(Failure {
                    code: error.code(),
                    message: error.to_string(),
                }),
                &[],
            )
            .map_err(ToolError::from)
        }
    };
    match sent {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn answer(
    backend: &impl Backend,
    provides: &[Capability],
    request: HostMessage,
    stop: &Stop,
    out: &mut impl Write,
) -> Result<(), ToolError> {
    let (capability, path, made) = match request {
        HostMessage::Decode(request) => (
            Capability::Decode,
            request.path.clone(),
            Fit::Area(request.max_area),
        ),
        HostMessage::Thumbnail(request) => (
            Capability::Thumbnail,
            request.path.clone(),
            Fit::Edge(request.max_edge),
        ),
        HostMessage::Probe(_) | HostMessage::Export(_) => {
            return Err(ToolError::Unsupported(
                "this plugin only makes pictures".to_owned(),
            ));
        }
        HostMessage::Cancel => return Ok(()),
    };
    if !provides.contains(&capability) {
        return Err(ToolError::Unsupported(format!(
            "this machine's tools cannot {}",
            capability.name()
        )));
    }
    std::fs::metadata(&path).map_err(|error| ToolError::Unreadable(error.kind().to_string()))?;
    let picture = match made {
        Fit::Area(area) => backend.decode(&path, area, stop)?,
        Fit::Edge(edge) => backend.thumbnail(&path, edge, stop)?,
    };
    let header = ImageHeader {
        width: picture.width,
        height: picture.height,
    };
    Ok(write_frame(
        out,
        &PluginMessage::Image(header),
        &picture.rgba,
    )?)
}
