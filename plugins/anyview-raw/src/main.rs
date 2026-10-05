//! The RAW plugin: a program that speaks plugin protocol v1 (ARCHITECTURE section 2l) and makes
//! pictures of camera raw files by running the person's own LibRaw `dcraw_emu` (or Dave Coffin's
//! `dcraw`). It links no LibRaw and develops nothing itself.
//!
//! What it relies on:
//! - `dcraw_emu` (LibRaw's sample program) writes its output beside its input, named after it
//!   (`in.cr2.ppm`, `in.cr2.thumb.jpg`), so the plugin runs it on a symbolic link inside a scratch
//!   folder and reads what appears there. `-w` takes the camera's white balance; the output is the
//!   8-bit sRGB PPM it writes by default, already turned upright by the file's orientation (LibRaw
//!   flips unless told not to).
//! - `dcraw -c` writes its picture to standard output; `-e` selects the embedded preview.
//! - A thumbnail is the embedded preview (`-e`): LibRaw's and dcraw's own extraction. A raw file
//!   with no embedded preview gets a scaled-down development instead.

mod backend;

use anyview_tool_kit::{Lookup, serve};
use std::process::ExitCode;

fn main() -> ExitCode {
    let lookup = Lookup::from_process(&["ANYVIEW_DCRAW_EMU", "ANYVIEW_DCRAW"]);
    serve(&backend::Raw::locate(&lookup))
}
