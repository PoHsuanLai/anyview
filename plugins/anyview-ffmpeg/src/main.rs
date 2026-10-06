//! The FFmpeg plugin: a program that speaks plugin protocol v1 (ARCHITECTURE section 2l) and does
//! its work by running the person's own `ffprobe` and `ffmpeg`. It links no libav and decodes
//! nothing itself. It is a plugin process and not a layer of the viewer, so it is the one place
//! that reads the environment and starts programs.

mod encoders;
mod error;
mod events;
mod export;
mod facts;
mod ffprobe;
mod keyframe;
mod picture;
mod plan;
mod serve;
mod target;
mod tools;
mod units;

use std::process::ExitCode;
use tools::Lookup;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("anyview-ffmpeg {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let lookup = Lookup::from_process(
        std::env::args().skip(1),
        std::env::var_os("ANYVIEW_FFMPEG"),
        std::env::var_os("ANYVIEW_FFPROBE"),
        std::env::var_os("PATH"),
    );
    serve::run(&lookup)
}
