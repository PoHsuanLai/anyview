//! The HEIF plugin: a program that speaks plugin protocol v1 (ARCHITECTURE section 2l) and makes
//! pictures of HEIC, HEIF and AVIF files by running the person's own libheif tools. It links no
//! libheif and decodes nothing itself.
//!
//! What it relies on, and the tools' own behaviour it does not undo:
//! - `heif-dec` (libheif 1.17 and later) or `heif-convert` (older) is run as `<tool> <input>
//!   <dir>/out.png`; the picture is the PNG it writes. A file of several pictures gets
//!   `out-1.png`, `out-2.png`…: the first is the primary image and is the one used.
//! - Orientation: the tools apply the file's own `irot` and `imir` transformations when they
//!   decode (libheif's default is to honour them), so the PNG is upright and this plugin applies
//!   nothing more. A libheif run with those transformations ignored would show a picture on its
//!   side.
//! - `heif-thumbnailer -s <edge> <input> <dir>/out.png` makes thumbnails; without it a thumbnail
//!   is a decode scaled down.

mod backend;

use anyview_tool_kit::{Lookup, serve};
use std::process::ExitCode;

fn main() -> ExitCode {
    let lookup = Lookup::from_process(&["ANYVIEW_HEIF_DEC", "ANYVIEW_HEIF_THUMBNAILER"]);
    serve(&backend::Heif::locate(&lookup))
}
