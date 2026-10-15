//! Where a file was left and how wide the left panel was, for the views: the store the host keeps,
//! asked through its hosting.

use super::desktop::Hosting;
use anyview_core::{FilePath, FileStamp, Resume, Source};
use anyview_ui::{PanelMemory, ResumeSource};
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

/// The host's memory of the left panel's width, as the views' [`PanelMemory`].
pub struct HostedPanel(pub Arc<dyn Hosting>);

impl std::fmt::Debug for HostedPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedPanel").finish_non_exhaustive()
    }
}

impl PanelMemory for HostedPanel {
    fn recall(&self) -> Option<u32> {
        self.0.panel_width()
    }

    fn keep(&self, width: u32) {
        self.0.keep_panel_width(width);
    }
}
