//! What the jobs of one run share: the PDF they are read from, drawn on one worker, and the
//! highlighter, which is slow to build and built only when a job needs it.

use crate::error::ExportError;
use crate::plugin::PluginPictures;
use anyview_core::{FilePath, Source};
use anyview_pdf::{PdfDocument, PdfWorker};
use anyview_text::Highlighter;
use std::sync::Arc;

/// The state of one run of jobs.
#[derive(Default)]
pub(crate) struct Session {
    pdf: Option<(FilePath, Arc<PdfDocument>)>,
    pub(crate) worker: PdfWorker,
    highlighter: Option<Highlighter>,
    /// The file being exported and the plugins that may decode it, when the program lent any.
    pub(crate) plugins: Option<(Source, Arc<dyn PluginPictures>)>,
}

impl Session {
    /// A run on the file `source`, whose picture `plugins` may decode.
    pub(crate) fn with_plugins(source: Source, plugins: Arc<dyn PluginPictures>) -> Session {
        Session {
            plugins: Some((source, plugins)),
            ..Session::default()
        }
    }

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
