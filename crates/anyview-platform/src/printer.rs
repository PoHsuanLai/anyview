//! Printing a PDF through the desktop's own dialog.

use crate::error::PlatformError;
use ds_core::word::Word;
use std::future::Future;

/// How a print request ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PrintOutcome {
    /// The dialog was accepted and the job handed over.
    Printed,
    /// The person closed the dialog.
    Cancelled,
    /// There is no print dialog to show; the caller opens the PDF in a viewer instead.
    NoDialog,
}

/// The title a print job shows in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobTitle(pub String);

/// Print a PDF.
pub trait Printer {
    /// Show the print dialog and, if the person accepts it, print `pdf`.
    fn print(
        &self,
        pdf: &[u8],
        title: &JobTitle,
    ) -> impl Future<Output = Result<PrintOutcome, PlatformError>> + Send;
}
