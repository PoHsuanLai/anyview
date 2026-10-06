//! How the fake plugin behaves, chosen by the arguments its manifest passes.

/// What the plugin does wrong, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Nothing: it answers every request.
    None,
    /// It says it speaks protocol 99.
    OtherVersion,
    /// It says hello and then lists nothing it can do.
    ProvidesNothing,
    /// It never says hello.
    Mute,
    /// It exits with status 3 as soon as it has a request, after a line on stderr.
    CrashOnRequest,
    /// It reads the request and never answers.
    HangOnRequest,
    /// It writes bytes that are not a frame.
    Garbage,
    /// It answers a picture request with the wrong number of pixels.
    ShortPicture,
    /// It exits in the middle of an export, after the first progress message.
    CrashInExport,
    /// It writes a megabyte to stderr with no newline, then answers as usual.
    StderrFlood,
    /// It answers a picture request with a header that announces 100 MiB of pixels, then sends
    /// none of them.
    HugePayload,
    /// It ignores `Cancel` and carries on.
    IgnoreCancel,
}

/// The fake plugin's settings.
#[derive(Debug, Clone, Copy)]
pub struct Behaviour {
    /// What it does wrong.
    pub fault: Fault,
}

impl Behaviour {
    /// The behaviour the arguments name: `--fault <name>`, or none.
    pub fn from_args(args: impl Iterator<Item = String>) -> Behaviour {
        let args: Vec<String> = args.collect();
        let name = args
            .iter()
            .position(|arg| arg == "--fault")
            .and_then(|at| args.get(at + 1))
            .map(String::as_str);
        let fault = match name {
            Some("other-version") => Fault::OtherVersion,
            Some("provides-nothing") => Fault::ProvidesNothing,
            Some("mute") => Fault::Mute,
            Some("crash-on-request") => Fault::CrashOnRequest,
            Some("hang-on-request") => Fault::HangOnRequest,
            Some("garbage") => Fault::Garbage,
            Some("short-picture") => Fault::ShortPicture,
            Some("crash-in-export") => Fault::CrashInExport,
            Some("stderr-flood") => Fault::StderrFlood,
            Some("huge-payload") => Fault::HugePayload,
            Some("ignore-cancel") => Fault::IgnoreCancel,
            Some(_) | None => Fault::None,
        };
        Behaviour { fault }
    }
}
