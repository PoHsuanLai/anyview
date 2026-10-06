//! The plugin's main loop: greet, take one request, answer it.

use crate::behaviour::{Behaviour, Fault};
use anyview_plugin_protocol::{
    Capability, DecodeRequest, Done, ErrorCode, ExportRequest, FactRow, FactsReply, Failure, Hello,
    HostMessage, ImageHeader, PROTOCOL_VERSION, PluginMessage, ProbeRequest, Progress,
    ThumbnailRequest, encode_frame, read_frame, write_frame,
};
use std::fs;
use std::io::{Stdout, Write};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

/// The picture `decode` starts from, before it is scaled to the budget: 24 megapixels.
const SOURCE: (u32, u32) = (6000, 4000);

/// How many steps an export takes, and how long each does.
const STEPS: u64 = 10;
const STEP: Duration = Duration::from_millis(40);

/// Serves the one request that arrives. The exit code says whether the plugin got that far.
pub fn run(behaviour: &Behaviour) -> ExitCode {
    let mut out = std::io::stdout();
    if behaviour.fault == Fault::Mute {
        std::thread::sleep(Duration::from_secs(60));
        return ExitCode::SUCCESS;
    }
    if write_frame(&mut out, &hello(behaviour), &[]).is_err() {
        return ExitCode::FAILURE;
    }
    let (requests, inbox) = mpsc::channel();
    // The host may send `Cancel` while an export runs, so stdin is read on a thread of its own.
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        while let Ok(frame) = read_frame::<_, HostMessage>(&mut stdin) {
            if requests.send(frame.message).is_err() {
                break;
            }
        }
    });
    let Ok(request) = inbox.recv() else {
        return ExitCode::SUCCESS;
    };
    match behaviour.fault {
        Fault::CrashOnRequest => {
            eprintln!("fake plugin: crashing on purpose");
            return ExitCode::from(3);
        }
        Fault::HangOnRequest => {
            std::thread::sleep(Duration::from_secs(60));
            return ExitCode::SUCCESS;
        }
        Fault::Garbage => {
            let _ = out.write_all(b"\x05\0\0\0\0\0\0\0{nope");
            let _ = out.flush();
            return ExitCode::SUCCESS;
        }
        Fault::StderrFlood => {
            let _ = std::io::stderr().write_all(&vec![b'x'; 1 << 20]);
        }
        Fault::HugePayload => {
            let header = ImageHeader {
                width: 1,
                height: 1,
            };
            let Ok(mut frame) = encode_frame(&PluginMessage::Image(header), &[]) else {
                return ExitCode::FAILURE;
            };
            // The frame's second length word is the payload's: claim 100 MiB and send none.
            frame[4..8].copy_from_slice(&(100u32 << 20).to_le_bytes());
            let _ = out.write_all(&frame);
            let _ = out.flush();
            std::thread::sleep(Duration::from_secs(60));
            return ExitCode::SUCCESS;
        }
        Fault::None
        | Fault::OtherVersion
        | Fault::ProvidesNothing
        | Fault::Mute
        | Fault::ShortPicture
        | Fault::CrashInExport
        | Fault::IgnoreCancel => {}
    }
    let sent = match request {
        HostMessage::Probe(request) => probe(&mut out, &request),
        HostMessage::Thumbnail(request) => thumbnail(&mut out, behaviour, &request),
        HostMessage::Decode(request) => decode(&mut out, &request),
        HostMessage::Export(request) => export(&mut out, behaviour, &request, &inbox),
        HostMessage::Cancel => Ok(()),
    };
    match sent {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

type Sent = Result<(), anyview_plugin_protocol::ProtocolError>;

fn hello(behaviour: &Behaviour) -> PluginMessage {
    let (protocol, provides) = match behaviour.fault {
        Fault::OtherVersion => (99, vec![Capability::Probe]),
        Fault::ProvidesNothing => (PROTOCOL_VERSION, Vec::new()),
        Fault::None
        | Fault::Mute
        | Fault::CrashOnRequest
        | Fault::HangOnRequest
        | Fault::Garbage
        | Fault::ShortPicture
        | Fault::CrashInExport
        | Fault::StderrFlood
        | Fault::HugePayload
        | Fault::IgnoreCancel => (
            PROTOCOL_VERSION,
            vec![
                Capability::Probe,
                Capability::Thumbnail,
                Capability::Decode,
                Capability::Export,
            ],
        ),
    };
    PluginMessage::Hello(Hello {
        protocol,
        name: "fake".to_owned(),
        provides,
        targets: Vec::new(),
    })
}

fn refuse(out: &mut Stdout, code: ErrorCode, message: &str) -> Sent {
    let failure = PluginMessage::Error(Failure {
        code,
        message: message.to_owned(),
    });
    write_frame(out, &failure, &[])
}

/// The first line of the file, or the reason it cannot be read.
fn first_line(path: &std::path::Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(text.lines().next().unwrap_or_default().to_owned())
}

fn probe(out: &mut Stdout, request: &ProbeRequest) -> Sent {
    match first_line(&request.path) {
        Ok(line) => {
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
            write_frame(out, &PluginMessage::Facts(reply), &[])
        }
        Err(reason) => refuse(out, ErrorCode::Unreadable, &reason),
    }
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

fn thumbnail(out: &mut Stdout, behaviour: &Behaviour, request: &ThumbnailRequest) -> Sent {
    let edge = request.max_edge.min(16);
    let header = ImageHeader {
        width: edge,
        height: edge,
    };
    let mut pixels = gradient(edge, edge);
    if behaviour.fault == Fault::ShortPicture {
        pixels.truncate(pixels.len() - 4);
    }
    write_frame(out, &PluginMessage::Image(header), &pixels)
}

fn decode(out: &mut Stdout, request: &DecodeRequest) -> Sent {
    let (mut width, mut height) = SOURCE;
    let area = u64::from(width) * u64::from(height);
    if area > request.max_area {
        let scale = (request.max_area as f64 / area as f64).sqrt();
        width = ((f64::from(width) * scale) as u32).max(1);
        height = ((f64::from(height) * scale) as u32).max(1);
    }
    write_frame(
        out,
        &PluginMessage::Image(ImageHeader { width, height }),
        &gradient(width, height),
    )
}

fn export(
    out: &mut Stdout,
    behaviour: &Behaviour,
    request: &ExportRequest,
    inbox: &mpsc::Receiver<HostMessage>,
) -> Sent {
    let Ok(text) = first_line(&request.input) else {
        return refuse(out, ErrorCode::Unreadable, "cannot read the input");
    };
    if request.target != "txt" {
        return refuse(out, ErrorCode::Unsupported, "only the txt target");
    }
    let partial = request.output.with_extension("partial");
    if fs::write(&partial, "partial").is_err() {
        return refuse(out, ErrorCode::Failed, "cannot write the output");
    }
    for done in 1..=STEPS {
        std::thread::sleep(STEP);
        let cancelled = matches!(inbox.try_recv(), Ok(HostMessage::Cancel));
        if cancelled && behaviour.fault != Fault::IgnoreCancel {
            let _ = fs::remove_file(&partial);
            return refuse(out, ErrorCode::Cancelled, "stopped");
        }
        if behaviour.fault == Fault::CrashInExport && done == 2 {
            eprintln!("fake plugin: crashing mid export");
            std::process::exit(4);
        }
        write_frame(
            out,
            &PluginMessage::Progress(Progress { done, total: STEPS }),
            &[],
        )?;
    }
    let renamed = fs::write(&request.output, format!("exported: {text}\n"))
        .and_then(|()| fs::remove_file(&partial));
    match renamed {
        Ok(()) => write_frame(out, &PluginMessage::Done(Done { output: None }), &[]),
        Err(_) => refuse(out, ErrorCode::Failed, "cannot finish the output"),
    }
}
