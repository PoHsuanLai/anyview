//! The text of the pages a worker has searched, kept for the next search: reading a page's text
//! is most of what a search costs, and a person refines a find a few times.

use crate::document::DocId;
use anyview_core::PageIndex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// The most characters of page text a worker keeps: 16 MiB, the text of several thousand pages.
const BUDGET: usize = 4 * 1024 * 1024;

/// Page texts by document and page, the oldest let go first once they are over the budget. A
/// document that is edited is a new document, so a text never outlives the pages it was read from.
#[derive(Debug)]
pub(crate) struct PageTexts {
    pages: HashMap<(DocId, PageIndex), Arc<[char]>>,
    order: VecDeque<(DocId, PageIndex)>,
    held: usize,
    budget: usize,
}

impl Default for PageTexts {
    fn default() -> Self {
        PageTexts {
            pages: HashMap::new(),
            order: VecDeque::new(),
            held: 0,
            budget: BUDGET,
        }
    }
}

impl PageTexts {
    /// The text kept for `page` of `doc`.
    pub(crate) fn get(&self, doc: DocId, page: PageIndex) -> Option<Arc<[char]>> {
        self.pages.get(&(doc, page)).cloned()
    }

    /// Keep `text` as the text of `page` of `doc`, letting the oldest pages go if that is over the
    /// budget. A page whose text alone is over it is not kept.
    pub(crate) fn keep(&mut self, doc: DocId, page: PageIndex, text: Arc<[char]>) {
        if text.len() > self.budget {
            return;
        }
        let key = (doc, page);
        self.held += text.len();
        match self.pages.insert(key, text) {
            Some(old) => self.held = self.held.saturating_sub(old.len()),
            None => self.order.push_back(key),
        }
        while self.held > self.budget {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(gone) = self.pages.remove(&oldest) {
                self.held = self.held.saturating_sub(gone.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PagePicture, PdfDocument, pdf_of_pictures};
    use anyview_core::{PixelLen, PixelSize};

    fn a_document() -> PdfDocument {
        let page = PagePicture::Pixels {
            size: PixelSize {
                width: PixelLen(1),
                height: PixelLen(1),
            },
            rgba: vec![255; 4],
        };
        PdfDocument::from_bytes(pdf_of_pictures(&[page]).expect("a page")).expect("it opens")
    }

    #[test]
    fn the_newest_pages_stay_within_the_budget_and_the_oldest_go() {
        let doc = a_document().id();
        let text = |chars: usize| -> Arc<[char]> { vec!['x'; chars].into() };
        let mut texts = PageTexts {
            budget: 5,
            ..PageTexts::default()
        };
        texts.keep(doc, PageIndex(0), text(2));
        texts.keep(doc, PageIndex(1), text(2));
        texts.keep(doc, PageIndex(2), text(2));
        // A page too big for the whole budget is not kept, and does not push the others out.
        texts.keep(doc, PageIndex(3), text(6));
        let kept: Vec<bool> = (0..4)
            .map(|at| texts.get(doc, PageIndex(at)).is_some())
            .collect();
        assert_eq!(kept, [false, true, true, false]);
    }
}
