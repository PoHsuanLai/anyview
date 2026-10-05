//! The kept versions of a file, for the views: the store the host keeps, asked through its
//! hosting.

use super::desktop::Hosting;
use anyview_core::FilePath;
use anyview_ui::{VersionRow, VersionSource};
use std::sync::Arc;

/// The host's kept versions, as the views' [`VersionSource`].
pub struct HostedVersions(pub Arc<dyn Hosting>);

impl std::fmt::Debug for HostedVersions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedVersions").finish_non_exhaustive()
    }
}

impl VersionSource for HostedVersions {
    fn list(&self, path: &FilePath) -> Vec<VersionRow> {
        self.0.kept_versions(path)
    }
}
