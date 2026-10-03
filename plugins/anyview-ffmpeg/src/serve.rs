//! The plugin's main loop: find the tools, greet, take one request, answer it.

use crate::encoders::Encoders;
use crate::error::FfmpegError;
use crate::events::{Event, forward_host};
use crate::export::export;
use crate::facts::rows;
use crate::ffprobe::probe;
use crate::picture::{Fit, picture};
use crate::tools::{Lookup, Tools};
use anyview_plugin_protocol::{
    Capability, DecodeRequest, FactsReply, Failure, Hello, HostMessage, ImageHeader,
    PROTOCOL_VERSION, PluginMessage, ProbeRequest, ThumbnailRequest, write_frame,
};
use std::io::Write;
use std::process::ExitCode;
use std::sync::mpsc;

/// The programs, found and checked, and what they can write.
struct Ready {
    tools: Tools,
    encoders: Encoders,
}

fn prepare(lookup: &Lookup) -> Result<Ready, FfmpegError> {
    let tools = Tools::locate(lookup)?;
    let encoders = Encoders::detect(&tools.ffmpeg)?;
    Ok(Ready { tools, encoders })
}

/// The greeting: what this machine can do. Without FFmpeg it lists nothing and the log says why.
fn hello(ready: &Result<Ready, FfmpegError>) -> Hello {
    let (provides, targets) = match ready {
        Ok(ready) => (
            vec![
                Capability::Probe,
                Capability::Thumbnail,
                Capability::Decode,
                Capability::Export,
            ],
            ready
                .encoders
                .targets()
                .into_iter()
                .map(|t| t.slug().to_owned())
                .collect(),
        ),
        Err(_) => (Vec::new(), Vec::new()),
    };
    Hello {
        protocol: PROTOCOL_VERSION,
        name: "ffmpeg".to_owned(),
        provides,
        targets,
    }
}

/// Serves the one request that arrives.
pub fn run(lookup: &Lookup) -> ExitCode {
    let ready = prepare(lookup);
    let mut out = std::io::stdout();
    if let Err(error) = &ready {
        eprintln!("anyview-ffmpeg: offering nothing: {error}");
    }
    if write_frame(&mut out, &PluginMessage::Hello(hello(&ready)), &[]).is_err() {
        return ExitCode::FAILURE;
    }
    let Ok(ready) = ready else {
        return ExitCode::SUCCESS;
    };
    let (sender, events) = mpsc::channel();
    let host = sender.clone();
    // The host may send `Cancel` while an export runs, so its pipe is read on a thread of its own.
    std::thread::spawn(move || forward_host(std::io::stdin().lock(), &host));
    let request = loop {
        match events.recv() {
            Ok(Event::Host(request)) => break request,
            Ok(Event::HostGone) | Err(_) => return ExitCode::SUCCESS,
            Ok(Event::Progress(_) | Event::FfmpegEnded) => {}
        }
    };
    let answered = answer(&ready, request, &events, &sender, &mut out);
    let sent = match answered {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("anyview-ffmpeg: {error}");
            write_frame(
                &mut out,
                &PluginMessage::Error(Failure {
                    code: error.code(),
                    message: error.to_string(),
                }),
                &[],
            )
            .map_err(FfmpegError::from)
        }
    };
    match sent {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn answer(
    ready: &Ready,
    request: HostMessage,
    events: &mpsc::Receiver<Event>,
    sender: &mpsc::Sender<Event>,
    out: &mut impl Write,
) -> Result<(), FfmpegError> {
    match request {
        HostMessage::Probe(ProbeRequest { path }) => {
            let probed = probe(&ready.tools.ffprobe, &path)?;
            let reply = PluginMessage::Facts(FactsReply {
                rows: rows(&probed),
            });
            Ok(write_frame(out, &reply, &[])?)
        }
        HostMessage::Thumbnail(ThumbnailRequest { path, max_edge }) => {
            send_picture(ready, &path, Fit::Edge(max_edge), out)
        }
        HostMessage::Decode(DecodeRequest { path, max_area }) => {
            send_picture(ready, &path, Fit::Area(max_area), out)
        }
        HostMessage::Export(request) => {
            let done = export(&ready.tools, &ready.encoders, &request, events, sender, out)?;
            Ok(write_frame(out, &PluginMessage::Done(done), &[])?)
        }
        HostMessage::Cancel => Ok(()),
    }
}

fn send_picture(
    ready: &Ready,
    path: &std::path::Path,
    fit: Fit,
    out: &mut impl Write,
) -> Result<(), FfmpegError> {
    let probed = probe(&ready.tools.ffprobe, path)?;
    let made = picture(&ready.tools.ffmpeg, path, &probed, fit)?;
    let header = ImageHeader {
        width: made.width,
        height: made.height,
    };
    Ok(write_frame(out, &PluginMessage::Image(header), &made.rgba)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::Target;

    #[test]
    fn a_greeting_without_ffmpeg_offers_nothing() {
        let none = hello(&Err(FfmpegError::ToolMissing {
            tool: "ffmpeg",
            looked: "nowhere".to_owned(),
        }));
        assert!(none.provides.is_empty() && none.targets.is_empty());
        assert_eq!(none.protocol, PROTOCOL_VERSION);
    }

    #[test]
    fn a_greeting_lists_only_what_the_encoders_write() {
        let ready = Ok(Ready {
            tools: Tools {
                ffmpeg: "/f".into(),
                ffprobe: "/p".into(),
                version: crate::tools::Version::Release(8),
            },
            encoders: Encoders::parse("---\n A....D flac x\n"),
        });
        let greeting = hello(&ready);
        assert_eq!(greeting.targets, ["trim", "audio-copy", "flac"]);
        assert!(greeting.provides.contains(&Capability::Export));
        assert_eq!(Target::parse(&greeting.targets[2]), Some(Target::Flac));
    }
}
