use super::locked;
use crate::error::PlatformError;
use crate::spawn::{Argv, Spawn};
use std::sync::{Arc, Mutex};

/// A [`Spawn`] that starts nothing and remembers what it was asked to start.
#[derive(Debug, Clone, Default)]
pub struct RecordingSpawn {
    started: Arc<Mutex<Vec<Argv>>>,
}

impl RecordingSpawn {
    /// Everything started so far, oldest first.
    pub fn started(&self) -> Vec<Argv> {
        locked(&self.started).clone()
    }
}

impl Spawn for RecordingSpawn {
    fn spawn(&self, argv: &Argv) -> Result<(), PlatformError> {
        locked(&self.started).push(argv.clone());
        Ok(())
    }
}
