//! Where a file was left, for the views: the store the host keeps, asked through its hosting.

use super::desktop::Hosting;
use anyview_core::{FilePath, FileStamp, Resume, Source};
use anyview_ui::ResumeSource;
use std::sync::Arc;

/// The host's memory of where each file was left, as the views' [`ResumeSource`].
pub struct HostedResume(pub Arc<dyn Hosting>);

impl std::fmt::Debug for HostedResume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedResume").finish_non_exhaustive()
    }
}

impl ResumeSource for HostedResume {
    fn recall(&self, path: &FilePath, stamp: FileStamp) -> Resume {
        self.0
            .resume(&Source::new(path.clone(), stamp))
            .unwrap_or(Resume::Nothing)
    }
}
