use super::locked;
use crate::error::PlatformError;
use crate::share::{Share, ShareTarget};
use anyview_core::FilePath;
use std::sync::{Arc, Mutex};

/// A [`Share`] with every target on offer that records what is sent where.
#[derive(Debug, Clone, Default)]
pub struct FakeShare {
    shared: Arc<Mutex<Vec<(FilePath, ShareTarget)>>>,
}

impl FakeShare {
    /// Every share, oldest first.
    pub fn shared(&self) -> Vec<(FilePath, ShareTarget)> {
        locked(&self.shared).clone()
    }
}

impl Share for FakeShare {
    fn targets(&self) -> Vec<ShareTarget> {
        vec![ShareTarget::Mail]
    }

    async fn share(&self, file: &FilePath, target: ShareTarget) -> Result<(), PlatformError> {
        locked(&self.shared).push((file.clone(), target));
        Ok(())
    }
}
