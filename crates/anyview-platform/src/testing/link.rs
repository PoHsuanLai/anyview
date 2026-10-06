use super::locked;
use crate::error::PlatformError;
use crate::link::OpenLink;
use std::sync::{Arc, Mutex};

/// An [`OpenLink`] that opens nothing and records the addresses it was given.
#[derive(Debug, Clone, Default)]
pub struct FakeLinks {
    opened: Arc<Mutex<Vec<String>>>,
}

impl FakeLinks {
    /// Every address opened, oldest first.
    pub fn opened(&self) -> Vec<String> {
        locked(&self.opened).clone()
    }
}

impl OpenLink for FakeLinks {
    fn open(&self, uri: &str) -> Result<(), PlatformError> {
        locked(&self.opened).push(uri.to_owned());
        Ok(())
    }
}
