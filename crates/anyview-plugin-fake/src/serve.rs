//! The plugin's main loop: bayonet's fake greets and misbehaves on the wire, this answers the one
//! request that arrives.

use anyview_plugin_protocol::{
    Capability, DecodeRequest, Done, ErrorCode, ExportRequest, FactRow, FactsReply, Failure, Hello,
    HostMessage, ImageHeader, PROTOCOL_VERSION, PluginMessage, ProbeRequest, Progress,
    ThumbnailRequest, WireError, encode_frame,
};
use bayonet::testing::{Conversation, Ending, Fake, Fault, Step, fault_argument};
use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;
use std::{fs, path::Path};

/// The picture `decode` starts from, before it is scaled to the budget: 24 megapixels.
const SOURCE: (u32, u32) = (6000, 4000);

/// How many steps an export takes, and how long each does.
const STEPS: u64 = 10;
const STEP: Duration = Duration::from_millis(40);

type Talk<'a, W> = Conversation<'a, W, HostMessage>;

/// The faults that need the viewer's messages; the rest are bayonet's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Own {
    /// A picture request is answered with one pixel too few.
    ShortPicture,
    /// The plugin exits in the middle of an export, after the first progress message.
    CrashInExport,
    /// A header announces 100 MiB of pixels, then none of them are sent.
    HugePayload,
}

impl Own {
    fn from_name(name: &str) -> Option<Own> {
        match name {
            "short-picture" => Some(Own::ShortPicture),
            "crash-in-export" => Some(Own::CrashInExport),
            "huge-payload" => Some(Own::HugePayload),
            _ => None,
        }
    }
}

/// Serves the one request that arrives. The exit code says whether the plugin got that far.
pub fn run() -> ExitCode {
    let name = fault_argument(std::env::args().skip(1));
    let own = name.as_deref().and_then(Own::from_name);
    Fake::new(
        PROTOCOL_VERSION,
        vec![
            Capability::Probe,
            Capability::Thumbnail,
            Capability::Decode,
            Capability::Export,
        ],
        |protocol: u32, provides: &[Capability]| {
            PluginMessage::Hello(Hello {
                protocol,
                name: "fake".to_owned(),
                provides: provides.to_vec(),
                targets: Vec::new(),
            })
        },
        |request: &HostMessage| matches!(request, HostMessage::Cancel),
    )
    .with_fault(name.as_deref().and_then(Fault::from_name))
    .run(|request, talk| answer(request, talk, own))
}

fn answer<W: Write>(
    request: HostMessage,
    talk: &mut Talk<'_, W>,
    own: Option<Own>,
) -> Result<Step, WireError> {
    if own == Some(Own::HugePayload) {
        let header = ImageHeader {
            width: 1,
            height: 1,
        };
        let mut frame = encode_frame(&PluginMessage::Image(header), &[])?;
        // The frame's second length word is the payload's: claim 100 MiB and send none.
        frame[4..8].copy_from_slice(&(100u32 << 20).to_le_bytes());
        talk.say_raw(&frame)?;
        return Ok(Step::End(Ending::Hung));
    }
    match request {
        HostMessage::Probe(request) => probe(talk, &request),
        HostMessage::Thumbnail(request) => thumbnail(talk, own, &request),
        HostMessage::Decode(request) => decode(talk, &request),
        HostMessage::Export(request) => export(talk, own, &request),
        HostMessage::Cancel => Ok(Step::End(Ending::Finished)),
    }
}

/// Says `message` with its `payload` and ends the plugin: each run answers once.
fn say<W: Write>(
    talk: &mut Talk<'_, W>,
    message: &PluginMessage,
    payload: &[u8],
) -> Result<Step, WireError> {
    talk.say(message, payload)?;
    Ok(Step::End(Ending::Finished))
}

fn refuse<W: Write>(
    talk: &mut Talk<'_, W>,
    code: ErrorCode,
    message: &str,
) -> Result<Step, WireError> {
    let failure = Failure {
        code,
        message: message.to_owned(),
    };
    say(talk, &PluginMessage::Error(failure), &[])
}

/// The first line of the file, or the reason it cannot be read.
fn first_line(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(text.lines().next().unwrap_or_default().to_owned())
}

fn probe<W: Write>(talk: &mut Talk<'_, W>, request: &ProbeRequest) -> Result<Step, WireError> {
    let line = match first_line(&request.path) {
        Ok(line) => line,
        Err(reason) => return refuse(talk, ErrorCode::Unreadable, &reason),
    };
    let row = |label: &str, value: String| FactRow {
        label: label.to_owned(),
        value,
    };
    let reply = FactsReply {
        rows: vec![
            row("kind", "Fake book".to_owned()),
            row("title", line),
            row("mood", "ignored by the viewer".to_owned()),
        ],
    };
    say(talk, &PluginMessage::Facts(reply), &[])
}

/// An RGBA picture whose pixel at (x, y) is `[x, y, 0, 255]`, each taken modulo 256.
fn gradient(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 0, 255]);
        }
    }
    pixels
}

fn thumbnail<W: Write>(
    talk: &mut Talk<'_, W>,
    own: Option<Own>,
    request: &ThumbnailRequest,
) -> Result<Step, WireError> {
    let edge = request.max_edge.min(16);
    let mut pixels = gradient(edge, edge);
    if own == Some(Own::ShortPicture) {
        pixels.truncate(pixels.len() - 4);
    }
    let header = ImageHeader {
        width: edge,
        height: edge,
    };
    say(talk, &PluginMessage::Image(header), &pixels)
}

fn decode<W: Write>(talk: &mut Talk<'_, W>, request: &DecodeRequest) -> Result<Step, WireError> {
    let (mut width, mut height) = SOURCE;
    let area = u64::from(width) * u64::from(height);
    if area > request.max_area {
        let scale = (request.max_area as f64 / area as f64).sqrt();
        width = ((f64::from(width) * scale) as u32).max(1);
        height = ((f64::from(height) * scale) as u32).max(1);
    }
    let header = ImageHeader { width, height };
    say(
        talk,
        &PluginMessage::Image(header),
        &gradient(width, height),
    )
}

fn export<W: Write>(
    talk: &mut Talk<'_, W>,
    own: Option<Own>,
    request: &ExportRequest,
) -> Result<Step, WireError> {
    let Ok(text) = first_line(&request.input) else {
        return refuse(talk, ErrorCode::Unreadable, "cannot read the input");
    };
    if request.target != "txt" {
        return refuse(talk, ErrorCode::Unsupported, "only the txt target");
    }
    let partial = request.output.with_extension("partial");
    if fs::write(&partial, "partial").is_err() {
        return refuse(talk, ErrorCode::Failed, "cannot write the output");
    }
    for done in 1..=STEPS {
        std::thread::sleep(STEP);
        if talk.cancelled() {
            let _ = fs::remove_file(&partial);
            return refuse(talk, ErrorCode::Cancelled, "stopped");
        }
        if own == Some(Own::CrashInExport) && done == 2 {
            let said = "fake plugin: crashing mid export";
            eprintln!("{said}");
            return Ok(Step::End(Ending::Crashed {
                code: 4,
                said: said.to_owned(),
            }));
        }
        let progress = Progress { done, total: STEPS };
        talk.say(&PluginMessage::Progress(progress), &[])?;
    }
    let renamed = fs::write(&request.output, format!("exported: {text}\n"))
        .and_then(|()| fs::remove_file(&partial));
    match renamed {
        Ok(()) => say(talk, &PluginMessage::Done(Done { output: None }), &[]),
        Err(_) => refuse(talk, ErrorCode::Failed, "cannot finish the output"),
    }
}
