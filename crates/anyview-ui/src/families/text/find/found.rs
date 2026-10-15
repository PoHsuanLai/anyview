//! Every hit of a find and where each is: a text's lines, a table's cells, a tree's nodes or a
//! rendered page, and the snippets the palette lists.

use super::snippet::Snippet;
use crate::families::places::{CellHit, NodeHit};
use crate::{HitCount, HitIndex, SheetNo};
use anyview_core::{LineIndex, TreePath};
use anyview_text::{FindHit, Needle};

/// Where the hits of a find are, by what the file is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Places {
    /// The matches in the lines of a text, in file order.
    Lines(Vec<FindHit>),
    /// The matches in the cells of a table's sheet, in file order.
    Cells {
        /// The sheet that was searched.
        sheet: SheetNo,
        /// The matches.
        hits: Vec<CellHit>,
    },
    /// The matches in the keys and values of a tree, in document order.
    Nodes(Vec<NodeHit>),
    /// The matches in the text of a rendered page, in reading order.
    Page {
        /// How many there are.
        count: u32,
        /// What marks them again when the page is drawn.
        needle: Needle,
    },
}

impl Default for Places {
    fn default() -> Self {
        Places::Lines(Vec::new())
    }
}

/// Every hit of a phrase in a file, in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FoundHits {
    places: Places,
    /// The snippets of the first hits, in the same order; the palette lists no more than these.
    snippets: Vec<Snippet>,
}

impl FoundHits {
    /// The hits `hits`, which a search made in file order.
    #[must_use]
    pub fn new(hits: Vec<FindHit>) -> FoundHits {
        FoundHits {
            places: Places::Lines(hits),
            snippets: Vec::new(),
        }
    }

    /// The hits `hits` in the cells of `sheet` of a table, in file order.
    pub(crate) fn cells(sheet: SheetNo, hits: Vec<CellHit>) -> FoundHits {
        FoundHits {
            places: Places::Cells { sheet, hits },
            snippets: Vec::new(),
        }
    }

    /// The sheet the hits are in, when they are places in the cells of a table.
    pub(crate) fn sheet(&self) -> Option<SheetNo> {
        match &self.places {
            Places::Cells { sheet, .. } => Some(*sheet),
            Places::Lines(_) | Places::Nodes(_) | Places::Page { .. } => None,
        }
    }

    /// The hits `hits` in the nodes of a tree, in document order.
    pub(crate) fn nodes(hits: Vec<NodeHit>) -> FoundHits {
        FoundHits {
            places: Places::Nodes(hits),
            snippets: Vec::new(),
        }
    }

    /// `count` hits in the text of a rendered page, which `needle` marks.
    pub(crate) fn page(count: u32, needle: Needle) -> FoundHits {
        FoundHits {
            places: Places::Page { count, needle },
            snippets: Vec::new(),
        }
    }

    /// The same hits with the words around the first of them.
    pub(crate) fn with_snippets(self, snippets: Vec<Snippet>) -> FoundHits {
        FoundHits { snippets, ..self }
    }

    /// The words around the hit numbered `index`, when they were kept.
    pub(crate) fn snippet(&self, index: HitIndex) -> Option<&Snippet> {
        self.snippets.get(index.0 as usize)
    }

    /// How many there are.
    #[must_use]
    pub fn count(&self) -> HitCount {
        let len = match &self.places {
            Places::Lines(hits) => hits.len(),
            Places::Cells { hits, .. } => hits.len(),
            Places::Nodes(hits) => hits.len(),
            Places::Page { count, .. } => *count as usize, // a u32 fits a usize
        };
        HitCount(u32::try_from(len).unwrap_or(u32::MAX))
    }

    /// The hit numbered `index`, when the hits are places in the lines of a text.
    #[must_use]
    pub fn get(&self, index: HitIndex) -> Option<&FindHit> {
        match &self.places {
            Places::Lines(hits) => hits.get(index.0 as usize),
            Places::Cells { .. } | Places::Nodes(_) | Places::Page { .. } => None,
        }
    }

    /// The hit numbered `index`, when the hits are places in the cells of a table.
    pub(crate) fn cell(&self, index: HitIndex) -> Option<&CellHit> {
        match &self.places {
            Places::Cells { hits, .. } => hits.get(index.0 as usize),
            Places::Lines(_) | Places::Nodes(_) | Places::Page { .. } => None,
        }
    }

    /// The hit numbered `index`, when the hits are places in the nodes of a tree.
    pub(crate) fn node(&self, index: HitIndex) -> Option<&NodeHit> {
        match &self.places {
            Places::Nodes(hits) => hits.get(index.0 as usize),
            Places::Lines(_) | Places::Cells { .. } | Places::Page { .. } => None,
        }
    }

    /// What marks the hits again in a rendered page, when they are places in one.
    pub(crate) fn page_needle(&self) -> Option<&Needle> {
        match &self.places {
            Places::Page { needle, .. } => Some(needle),
            Places::Lines(_) | Places::Cells { .. } | Places::Nodes(_) => None,
        }
    }

    /// The hit a reader at `line` meets first: the first on or after it, or the first of all when
    /// they are all behind. A table's hits are on its rows and a tree's or a page's have no line:
    /// the first of them.
    #[must_use]
    pub fn nearest(&self, line: LineIndex) -> HitIndex {
        let (at, len) = match &self.places {
            Places::Lines(hits) => (hits.partition_point(|hit| hit.line < line), hits.len()),
            Places::Cells { hits, .. } => {
                (hits.partition_point(|hit| hit.row < line.0), hits.len())
            }
            Places::Nodes(_) | Places::Page { .. } => return HitIndex(0),
        };
        match u32::try_from(at) {
            Ok(at) if (at as usize) < len => HitIndex(at),
            Ok(_) | Err(_) => HitIndex(0),
        }
    }

    /// The hits on `line`, and the number of the first of them.
    #[must_use]
    pub fn on_line(&self, line: LineIndex) -> (HitIndex, &[FindHit]) {
        let Places::Lines(hits) = &self.places else {
            return (HitIndex(0), &[]);
        };
        let start = hits.partition_point(|hit| hit.line < line);
        let end = hits.partition_point(|hit| hit.line <= line);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &hits[start..end],
        )
    }

    /// The hits in the cells of `row`, and the number of the first of them.
    pub(crate) fn in_row(&self, row: u32) -> (HitIndex, &[CellHit]) {
        let Places::Cells { hits, .. } = &self.places else {
            return (HitIndex(0), &[]);
        };
        let start = hits.partition_point(|hit| hit.row < row);
        let end = hits.partition_point(|hit| hit.row <= row);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &hits[start..end],
        )
    }

    /// The hits in the node at `path`, and the number of the first of them.
    pub(crate) fn at_node(&self, path: &TreePath) -> (HitIndex, &[NodeHit]) {
        let Places::Nodes(hits) = &self.places else {
            return (HitIndex(0), &[]);
        };
        let start = hits.partition_point(|hit| hit.path < *path);
        let end = hits.partition_point(|hit| hit.path <= *path);
        (
            HitIndex(u32::try_from(start).unwrap_or(u32::MAX)),
            &hits[start..end],
        )
    }
}

/// Where `hit` is, as the palette says it: the line, and for a line with more than one hit the
/// column too, so the rows of those hits tell apart.
pub(in crate::families::text) fn hit_place(
    found: &FoundHits,
    hit: &FindHit,
    snippet: &Snippet,
) -> String {
    let (_, on_line) = found.on_line(hit.line);
    let line = hit.line.0 + 1;
    if on_line.len() > 1 && snippet.column > 0 {
        format!("Line {line}, column {}", snippet.column)
    } else {
        format!("Line {line}")
    }
}

/// The line at the top that shows the hit on `hit`: where the view is when the hit is already in
/// the page, otherwise the hit a third of the way down the page.
pub(crate) fn top_for(hit: LineIndex, top: LineIndex, page: u32) -> LineIndex {
    let page = page.max(1);
    if hit.0 >= top.0 && hit.0 < top.0.saturating_add(page) {
        top
    } else {
        LineIndex(hit.0.saturating_sub(page / 3))
    }
}
