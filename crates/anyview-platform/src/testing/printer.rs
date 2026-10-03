use super::locked;
use crate::error::PlatformError;
use crate::printer::{JobTitle, PrintOutcome, Printer};
use std::sync::{Arc, Mutex};

/// A [`Printer`] that answers every job with one outcome and records the jobs.
#[derive(Debug, Clone)]
pub struct FakePrinter {
    outcome: PrintOutcome,
    jobs: Arc<Mutex<Vec<(JobTitle, usize)>>>,
}

impl FakePrinter {
    /// A printer that ends every job in `outcome`.
    pub fn answering(outcome: PrintOutcome) -> Self {
        FakePrinter {
            outcome,
            jobs: Arc::default(),
        }
    }

    /// Each job's title and the length of its PDF, oldest first.
    pub fn jobs(&self) -> Vec<(JobTitle, usize)> {
        locked(&self.jobs).clone()
    }
}

impl Printer for FakePrinter {
    async fn print(&self, pdf: &[u8], title: &JobTitle) -> Result<PrintOutcome, PlatformError> {
        locked(&self.jobs).push((title.clone(), pdf.len()));
        Ok(self.outcome)
    }
}
