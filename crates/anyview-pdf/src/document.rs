//! An open PDF, shared between workers.

use crate::error::PdfError;
use crate::geometry::{MilliPoints, PageSize};
use anyview_core::{PageCount, PageIndex};
use pdfrum::Document;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Which open document this is. A worker keeps caches that belong to one document (fonts and
/// images are keyed by object number), so it checks this before reusing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocId(u64);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A PDF opened once and read from any number of workers: put it in an `Arc` and hand each worker
/// a reference. It is `Send + Sync`; nothing in it changes after opening.
#[derive(Debug)]
pub struct PdfDocument {
    id: DocId,
    doc: Arc<Document>,
    count: PageCount,
    sizes: Vec<PageSize>,
}

impl PdfDocument {
    /// Opens the file at `path`. An encrypted file is [`PdfError::Locked`] until
    /// [`PdfDocument::open_with_password`] is given its password.
    pub fn open(path: &Path) -> Result<PdfDocument, PdfError> {
        PdfDocument::wrap(Document::open(path)?)
    }

    /// Opens the encrypted file at `path` with `password`.
    pub fn open_with_password(path: &Path, password: &str) -> Result<PdfDocument, PdfError> {
        PdfDocument::wrap(Document::open_with_password(path, password)?)
    }

    /// Opens a PDF held in memory (what an edit writes).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<PdfDocument, PdfError> {
        PdfDocument::wrap(Document::from_bytes(bytes)?)
    }

    fn wrap(doc: Document) -> Result<PdfDocument, PdfError> {
        let count = PageCount::new(doc.page_count()).ok_or(PdfError::NoPages)?;
        // A page that will not load keeps its place in the layout at Letter size; drawing it is
        // where the failure is reported.
        let sizes = (0..count.get())
            .map(|index| {
                doc.page(index).map_or(PageSize::LETTER, |page| PageSize {
                    width: MilliPoints::from_points(page.width()),
                    height: MilliPoints::from_points(page.height()),
                })
            })
            .collect();
        Ok(PdfDocument {
            id: DocId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            doc: Arc::new(doc),
            count,
            sizes,
        })
    }

    /// Which document this is.
    pub fn id(&self) -> DocId {
        self.id
    }

    /// How many pages the document has; never zero.
    pub fn page_count(&self) -> PageCount {
        self.count
    }

    /// Every page's displayed size, in page order.
    pub fn page_sizes(&self) -> &[PageSize] {
        &self.sizes
    }

    /// One page's displayed size.
    pub fn page_size(&self, page: PageIndex) -> Result<PageSize, PdfError> {
        usize::try_from(page.0)
            .ok()
            .and_then(|at| self.sizes.get(at))
            .copied()
            .ok_or(PdfError::PageOutOfRange {
                page,
                count: self.count.get(),
            })
    }

    /// Whether the file carries a digital signature, which a rewrite of the file would break.
    pub fn is_signed(&self) -> bool {
        signed(self.bytes())
    }

    /// The file's bytes as opened.
    pub fn bytes(&self) -> &[u8] {
        self.doc.bytes()
    }

    pub(crate) fn inner(&self) -> &Document {
        &self.doc
    }

    /// The page, or why there is none.
    pub(crate) fn page(&self, page: PageIndex) -> Result<pdfrum::Page<'_>, PdfError> {
        self.page_size(page)?;
        Ok(self.doc.page(page.0)?)
    }

    /// The page as a handle that keeps the document alive, for a prepared page a worker holds
    /// between jobs; or why there is none.
    pub(crate) fn owned_page(&self, page: PageIndex) -> Result<pdfrum::OwnedPage, PdfError> {
        self.page_size(page)?;
        Ok(self.doc.page_owned(page.0)?)
    }
}

/// Whether `bytes` holds a signature: a signature dictionary names the bytes it covers with
/// `/ByteRange`, and a signature field has the field type `/Sig`. Signature dictionaries are never
/// packed into object streams, so a scan of the file finds them.
fn signed(bytes: &[u8]) -> bool {
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|window| window == needle);
    has(b"/ByteRange") || has(b"/FT/Sig") || has(b"/FT /Sig")
}
