//! What the jobs of one run share: the PDF they are read from, drawn on one worker, and the
//! highlighter, which is slow to build and built only when a job needs it.

use crate::error::ExportError;
use anyview_core::FilePath;
use anyview_pdf::{PdfDocument, PdfWorker};
use anyview_text::Highlighter;
use std::sync::Arc;

/// The state of one run of jobs.
#[derive(Default)]
pub(crate) struct Session {
    pdf: Option<(FilePath, Arc<PdfDocument>)>,
    pub(crate) worker: PdfWorker,
    highlighter: Option<Highlighter>,
}

impl Session {
    /// The PDF at `file`, opened the first time it is asked for.
    pub(crate) fn document(&mut self, file: &FilePath) -> Result<Arc<PdfDocument>, ExportError> {
        if let Some((open, doc)) = &self.pdf
            && open == file
        {
            return Ok(Arc::clone(doc));
        }
        let doc = Arc::new(PdfDocument::open(file.as_path())?);
        self.pdf = Some((file.clone(), Arc::clone(&doc)));
        Ok(doc)
    }

    /// The syntaxes the highlighter knows.
    pub(crate) fn highlighter(&mut self) -> &Highlighter {
        self.highlighter.get_or_insert_with(Highlighter::new)
    }
}
