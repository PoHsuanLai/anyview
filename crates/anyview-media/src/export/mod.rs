//! Writing recordings with libav: a trim cut at a keyframe, a track lifted out, audio converted to
//! another format. Each is blocking work for one pool worker, with progress and a `Stop`; this
//! crate plans and names the work and runs it, and the binary owns the thread.

mod convert;
mod copy;
mod encoders;
mod fifo;
mod flac;
mod gate;
mod naming;
mod plan;
mod request;
mod run;
#[cfg(test)]
mod tests;

pub use encoders::{AudioFormat, Encoder, Encoders};
pub use naming::output_path;
pub use plan::plan_export;
pub use request::{ExportProgress, ExportReport, ExportRequest, ProgressSink};
pub use run::ExportBackend;
