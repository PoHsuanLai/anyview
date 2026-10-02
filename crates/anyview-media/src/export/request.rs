//! What a media export is asked, how far it has got and what it made.

use anyview_core::{ExportJob, FilePath, MediaLength, MediaTime};
use std::sync::Arc;

/// How far an export has got, in the recording's own time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportProgress {
    /// The part of the recording written.
    pub done: MediaTime,
    /// The part the export will write, when the recording says how long it runs.
    pub of: Option<MediaLength>,
}

/// Where an export reports its progress: called on the worker, so it must only post.
pub type ProgressSink = Arc<dyn Fn(ExportProgress) + Send + Sync>;

/// One export to run: the job, the file to write and where progress goes.
#[derive(Clone)]
pub struct ExportRequest {
    /// What to do; only `ExportJob::Transcode` is libav's.
    pub job: ExportJob,
    /// The file to write. It is made whole or not at all.
    pub to: FilePath,
    /// Told of progress, never less than it was told before.
    pub progress: ProgressSink,
}

impl std::fmt::Debug for ExportRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportRequest")
            .field("job", &self.job)
            .field("to", &self.to)
            .finish_non_exhaustive()
    }
}

/// What a finished export made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    /// The file written.
    pub path: FilePath,
    /// How much of the recording it holds.
    pub length: MediaLength,
}
