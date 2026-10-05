//! What the picture plugins share (ARCHITECTURE section 2l). A picture plugin is a program that
//! speaks plugin protocol v1 and does its work by running the person's own command line tools
//! (libheif's, LibRaw's), reading the picture they write and sending it as RGBA. It links no codec
//! library: `image` here reads only the PNG, TIFF and PPM files the tools produce.
//!
//! The plugin is a process and not a layer of the viewer, so it is the one place that reads the
//! environment and starts programs.

mod error;
mod lookup;
mod picture;
mod run;
mod serve;

pub use error::ToolError;
pub use lookup::{Lookup, find_tool};
pub use picture::{Fit, Picture, fitted, load_picture, scaled};
pub use run::{Ran, Stop, run_tool};
pub use serve::{Backend, serve};
