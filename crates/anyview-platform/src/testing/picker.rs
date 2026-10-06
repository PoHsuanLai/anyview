use super::locked;
use crate::error::PlatformError;
use crate::picker::{PickOutcome, Picker};
use std::sync::{Arc, Mutex};

/// A [`Picker`] that answers every request with one outcome and counts the requests.
#[derive(Debug, Clone)]
pub struct FakePicker {
    outcome: PickOutcome,
    asked: Arc<Mutex<u32>>,
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
        *locked(&self.asked)
    }
}

impl Picker for FakePicker {
    async fn pick(&self) -> Result<PickOutcome, PlatformError> {
        *locked(&self.asked) += 1;
        Ok(self.outcome.clone())
    }
}
