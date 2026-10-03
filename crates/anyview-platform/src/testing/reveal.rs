use super::locked;
use crate::error::PlatformError;
use crate::reveal::Reveal;
use anyview_core::FilePath;
use std::sync::{Arc, Mutex};

/// A [`Reveal`] that records the files it was asked to show.
#[derive(Debug, Clone, Default)]
pub struct FakeReveal {
    revealed: Arc<Mutex<Vec<FilePath>>>,
}

impl FakeReveal {
    /// Every file revealed, oldest first.
    pub fn revealed(&self) -> Vec<FilePath> {
        locked(&self.revealed).clone()
    }
}

impl Reveal for FakeReveal {
    async fn reveal(&self, file: &FilePath) -> Result<(), PlatformError> {
        locked(&self.revealed).push(file.clone());
        Ok(())
    }
}
