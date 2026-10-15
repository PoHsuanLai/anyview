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
        Edit::Flip(_) | Edit::Adjust(_) | Edit::Trim { .. } => {
            Err(PdfError::EditUnsupported { kind: edit.kind() })
        }
    }
}

/// The document with `ops` applied in order, as the bytes of a new file; the open document is
/// untouched. Each edit sees the pages as the one before it left them. The chain is folded into
/// one page order, a set of deleted pages and a turn for each page, and the file is written
/// once. Nothing is written if any edit fails.
pub fn apply(doc: &PdfDocument, ops: &[PageOp]) -> Result<Vec<u8>, PdfError> {
    if ops.is_empty() {
        return Ok(doc.bytes().to_vec());
    }
    let folded = Folded::of(doc.page_count().get(), ops)?;
    let bytes = write(doc, &folded)?;
    let (want, got) = (
        u32::try_from(folded.kept.len()).unwrap_or(u32::MAX),
        PdfDocument::from_bytes(bytes.clone())?.page_count().get(),
    );
    if want != got {
        return Err(PdfError::PagesChanged { want, got });
    }
    Ok(bytes)
}

/// What a chain of edits comes to, by the numbering of the document as it was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Folded {
    /// The pages that stay, in the order they end in, each as its original index.
    kept: Vec<u32>,
    /// The pages that go, as original indices, in the order they were deleted.
    doomed: Vec<u32>,
    /// The degrees added to each page's turn, by original index.
    turns: Vec<u32>,
}

impl Folded {
    /// Plays `ops` over a document of `count` pages without writing anything.
    fn of(count: u32, ops: &[PageOp]) -> Result<Folded, PdfError> {
        let mut folded = Folded {
            kept: (0..count).collect(),
            doomed: Vec::new(),
            turns: vec![0; count as usize],
        };
        for op in ops {
            folded.play(*op)?;
        }
        Ok(folded)
    }

    /// The place in the current order that `page` names.
    fn place(&self, page: PageIndex) -> Result<usize, PdfError> {
        let at = page.0 as usize;
        if at < self.kept.len() {
            Ok(at)
        } else {
            Err(PdfError::PageOutOfRange {
                page,
                count: u32::try_from(self.kept.len()).unwrap_or(u32::MAX),
            })
        }
    }

    fn play(&mut self, op: PageOp) -> Result<(), PdfError> {
        match op {
            PageOp::Rotate { page, turn } => {
                let original = self.kept[self.place(page)?] as usize;
                self.turns[original] = (self.turns[original] + u32::from(turn.degrees())) % 360;
            }
            PageOp::Delete(range) => {
                let first = self.place(range.first())?;
                let last = (range.last().0 as usize).min(self.kept.len() - 1);
                if first == 0 && last == self.kept.len() - 1 {
                    return Err(PdfError::WouldDeleteAll);
                }
                for original in self.kept.drain(first..=last) {
                    self.turns[original as usize] = 0;
                    self.doomed.push(original);
                }
            }
            PageOp::Move { from, to } => {
                let (from, to) = (self.place(from)?, self.place(to)?);
                let moved = self.kept.remove(from);
                self.kept.insert(to, moved);
            }
        }
        Ok(())
    }

    /// The whole arrangement as pdfrum takes it: the pages that stay in their order, then the
    /// pages that go.
    fn order(&self) -> Vec<usize> {
        self.kept
            .iter()
            .chain(&self.doomed)
            .map(|&original| original as usize)
            .collect()
    }
}

/// Writes the folded edits as one new file. pdfrum numbers every call by the document as it was
/// opened, so the pages are put in order first, then the doomed are unlinked, then the turns are
/// set (a reorder rebuilds the page dictionaries from the file, so it must come before them).
fn write(doc: &PdfDocument, folded: &Folded) -> Result<Vec<u8>, PdfError> {
    let mut edit = EditDoc::new(doc.inner().parser());
    let order = folded.order();
    if order
        .iter()
        .enumerate()
        .any(|(at, &original)| at != original)
    {
        pdfrum_edit::reorder_pages(&mut edit, &order)?;
    }
    if !folded.doomed.is_empty() {
        let doomed = pdfrum_edit::PageRange::of(folded.doomed.iter().copied());
        pdfrum_edit::delete_pages(&mut edit, &doomed)?;
    }
    for &original in &folded.kept {
        let added = folded.turns[original as usize];
        if added == 0 {
            continue;
        }
        let now = doc.page(PageIndex(original))?.rotation().degrees();
        let degrees = i32::try_from(now + added).unwrap_or(0);
        pdfrum_edit::set_page_rotation(&mut edit, original, degrees)?;
    }
    save(&edit)
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
            let moved = PageOp::Move {
                from: PageIndex(*from),
                to: PageIndex(*to),
            };
            let got = Folded::of(*count, &[moved]).unwrap().order();
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
