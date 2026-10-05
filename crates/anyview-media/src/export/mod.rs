//! Writing recordings through a plugin: which choices a recording offers, what each asks of the
//! plugin that writes it (`ask`), where the file goes (`naming`) and what a request and its
//! progress are (`request`). Nothing here runs a program: the binary asks the FFmpeg plugin and
//! owns the thread that waits for it. A trim is a stream copy cut at the keyframe at or before
//! its start, written beside the file.

mod ask;
mod naming;
mod plan;
mod request;

pub use ask::{Ask, ask_of, offered_kinds, target_of};
pub use naming::{NameHints, output_path};
pub use plan::plan_export;
pub use request::{ExportProgress, ExportReport, ExportRequest, ProgressSink};
