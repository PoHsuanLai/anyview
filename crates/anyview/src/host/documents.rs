//! The documents half of the desktop's tasks: writing an export of an image, a PDF or a text
//! document beside it, and the PDF a printer takes for any of them.

use super::outcome::Outcome;
use anyview_export::{DocumentExport, ExportError};
use anyview_ui::Opened;

/// Write `choice` for `file` beside it. Blocking: the encode, the page layout or the drawing of
/// every page runs here, on a worker.
pub(super) fn export(file: &Opened, choice: DocumentExport) -> Outcome {
    match anyview_export::export(&file.source, &file.sniffed, choice) {
        Ok(written) => written
            .into_iter()
            .next()
            .map_or(Outcome::Done, Outcome::Wrote),
        Err(error) => Outcome::Failed(format!("cannot write the export: {error}")),
    }
}

/// The PDF that prints `file`. Blocking.
pub(super) fn printout(file: &Opened) -> Result<Vec<u8>, ExportError> {
    anyview_export::printout(&file.source, &file.sniffed)
}
