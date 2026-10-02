//! Page edits as data, and the bytes they write. An edit never changes the open document: it
//! returns the whole new file, which the save pipeline backs up and writes in place, and the
//! viewer then opens again.

use crate::document::PdfDocument;
use crate::error::PdfError;
use anyview_core::{Edit, PageIndex, PageRange, QuarterTurn};
use pdfrum_edit::{EditDoc, SaveOptions};

/// One change to a PDF's pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageOp {
    /// Turn one page clockwise.
    Rotate {
        /// The page.
        page: PageIndex,
        /// How far, added to the turn it already has.
        turn: QuarterTurn,
    },
    /// Remove a run of pages.
    Delete(PageRange),
    /// Move a page so it ends up at `to`; the pages between shift by one.
    Move {
        /// Where it is now.
        from: PageIndex,
        /// Where it will be.
        to: PageIndex,
    },
}

/// The page edit a viewer [`Edit`] means. `current` is the page the person is on, which a rotation
/// turns. A flip is not an edit a PDF takes.
pub fn page_op(edit: Edit, current: PageIndex) -> Result<PageOp, PdfError> {
    match edit {
        Edit::Rotate(turn) => Ok(PageOp::Rotate {
            page: current,
            turn,
        }),
        Edit::DeletePages(range) => Ok(PageOp::Delete(range)),
        Edit::MovePage { from, to } => Ok(PageOp::Move { from, to }),
        Edit::Flip(_) => Err(PdfError::EditUnsupported { kind: edit.kind() }),
    }
}

/// The document with `ops` applied in order, as the bytes of a new file; the open document is
/// untouched. Each edit sees the pages as the one before it left them. Nothing is written if any
/// edit fails.
pub fn apply(doc: &PdfDocument, ops: &[PageOp]) -> Result<Vec<u8>, PdfError> {
    let mut written: Option<(PdfDocument, Vec<u8>)> = None;
    for op in ops {
        let from = written.as_ref().map_or(doc, |(reopened, _)| reopened);
        let bytes = write_one(from, *op)?;
        written = Some((PdfDocument::from_bytes(bytes.clone())?, bytes));
    }
    Ok(written.map_or_else(|| doc.bytes().to_vec(), |(_, bytes)| bytes))
}

fn write_one(doc: &PdfDocument, op: PageOp) -> Result<Vec<u8>, PdfError> {
    let count = doc.page_count().get();
    let mut edit = EditDoc::new(doc.inner().parser());
    match op {
        PageOp::Rotate { page, turn } => {
            let now = doc.page(page)?.rotation().degrees();
            let degrees = i32::try_from(now + u32::from(turn.degrees())).unwrap_or(0);
            pdfrum_edit::set_page_rotation(&mut edit, page.0, degrees)?;
        }
        PageOp::Delete(range) => {
            doc.page_size(range.first())?;
            let last = range.last().0.min(count - 1);
            if range.first().0 == 0 && last == count - 1 {
                return Err(PdfError::WouldDeleteAll);
            }
            let doomed = pdfrum_edit::PageRange::of(range.first().0..=last);
            pdfrum_edit::delete_pages(&mut edit, &doomed)?;
        }
        PageOp::Move { from, to } => {
            doc.page_size(from)?;
            doc.page_size(to)?;
            pdfrum_edit::reorder_pages(&mut edit, &order_after_move(count, from, to))?;
        }
    }
    save(&edit)
}

/// The original index of each page after `from` moves to `to`.
fn order_after_move(count: u32, from: PageIndex, to: PageIndex) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count as usize).collect();
    let moved = order.remove(from.0 as usize);
    order.insert(to.0 as usize, moved);
    order
}

/// Writes an edit session as a whole new file.
pub(crate) fn save(edit: &EditDoc<'_>) -> Result<Vec<u8>, PdfError> {
    let mut out = Vec::new();
    pdfrum_edit::save(edit, &SaveOptions::default(), &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_names_the_original_page_at_each_place() {
        const CASES: &[(&str, u32, u32, u32, &[usize])] = &[
            // name, pages, from, to, order
            ("forward", 4, 0, 2, &[1, 2, 0, 3]),
            ("backward", 4, 3, 1, &[0, 3, 1, 2]),
            ("to the end", 3, 0, 2, &[1, 2, 0]),
            ("in place", 3, 1, 1, &[0, 1, 2]),
        ];
        for (name, count, from, to, want) in CASES {
            let got = order_after_move(*count, PageIndex(*from), PageIndex(*to));
            assert_eq!(&got, want, "{name}");
        }
    }

    #[test]
    fn a_viewer_edit_becomes_a_page_op() {
        use anyview_core::Axis;
        let here = PageIndex(3);
        let range = PageRange::new(PageIndex(1), PageIndex(2)).unwrap();
        assert_eq!(
            page_op(Edit::Rotate(QuarterTurn::Quarter), here).unwrap(),
            PageOp::Rotate {
                page: here,
                turn: QuarterTurn::Quarter
            }
        );
        assert_eq!(
            page_op(Edit::DeletePages(range), here).unwrap(),
            PageOp::Delete(range)
        );
        assert_eq!(
            page_op(
                Edit::MovePage {
                    from: here,
                    to: PageIndex(0)
                },
                here
            )
            .unwrap(),
            PageOp::Move {
                from: here,
                to: PageIndex(0)
            }
        );
        assert!(matches!(
            page_op(Edit::Flip(Axis::Vertical), here),
            Err(PdfError::EditUnsupported { .. })
        ));
    }
}
