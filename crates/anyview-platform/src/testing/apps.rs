use super::locked;
use crate::apps::{AppEntry, AppsForType, DesktopId};
use crate::error::PlatformError;
use anyview_core::{FilePath, Mime};
use std::sync::{Arc, Mutex};

/// An [`AppsForType`] that offers the entries it was given for every type and records what is
/// opened with what.
#[derive(Debug, Clone, Default)]
pub struct FakeApps {
    entries: Vec<AppEntry>,
    opened: Arc<Mutex<Vec<(DesktopId, FilePath)>>>,
}

impl FakeApps {
    /// A fake offering `entries`, in that order.
    pub fn offering(entries: Vec<AppEntry>) -> Self {
        FakeApps {
            entries,
            opened: Arc::default(),
        }
    }

    /// Every `open_with` call, oldest first.
    pub fn opened(&self) -> Vec<(DesktopId, FilePath)> {
        locked(&self.opened).clone()
    }
}

impl AppsForType for FakeApps {
    fn apps_for(&self, _mime: &Mime) -> Vec<AppEntry> {
        self.entries.clone()
    }

    fn open_with(&self, app: &DesktopId, file: &FilePath) -> Result<(), PlatformError> {
        locked(&self.opened).push((app.clone(), file.clone()));
        Ok(())
    }
}
