use super::locked;
use crate::error::PlatformError;
use crate::picker::{FileKinds, PickOutcome, Picker};
use std::sync::{Arc, Mutex};

/// A [`Picker`] that answers every request with one outcome and counts the requests.
#[derive(Debug, Clone)]
pub struct FakePicker {
    outcome: PickOutcome,
    asked: Arc<Mutex<Vec<FileKinds>>>,
}

impl FakePicker {
    /// A picker that ends every request in `outcome`.
    pub fn answering(outcome: PickOutcome) -> Self {
        FakePicker {
            outcome,
            asked: Arc::default(),
        }
    }

    /// How many times the dialog was asked for.
    pub fn asked(&self) -> u32 {
        u32::try_from(locked(&self.asked).len()).unwrap_or(u32::MAX)
    }

    /// The files each request offered first, oldest first.
    pub fn offered(&self) -> Vec<FileKinds> {
        locked(&self.asked).clone()
    }
}

impl Picker for FakePicker {
    async fn pick(&self, kinds: &FileKinds) -> Result<PickOutcome, PlatformError> {
        locked(&self.asked).push(kinds.clone());
        Ok(self.outcome.clone())
    }
}
