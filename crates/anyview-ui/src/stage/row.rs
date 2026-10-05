//! A row of a list the stage shows, counted from the top.

/// A row's position among the rows a table or a tree shows, zero-based. A tree's rows are its
/// visible nodes, so the same position names another node once something above it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RowNo(pub u32);

/// A move of the cursor along the rows of a table or a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStep {
    /// One row up.
    Up,
    /// One row down.
    Down,
    /// A page up.
    PageUp,
    /// A page down.
    PageDown,
    /// The first row.
    Top,
    /// The last row.
    Bottom,
}

impl RowStep {
    /// The row the cursor is on after this move from `from` (`None`: no cursor yet) over `rows`
    /// rows with `page` rows to a page. The first move with no cursor lands on the first row a
    /// move that way reaches; a list with no rows has no cursor.
    pub fn from(self, from: Option<RowNo>, rows: u32, page: u32) -> Option<RowNo> {
        let last = rows.checked_sub(1)?;
        let page = page.max(1);
        let to = match (self, from) {
            (RowStep::Top, _) | (RowStep::Up | RowStep::PageUp, None) | (RowStep::Down, None) => 0,
            (RowStep::Bottom, _) => last,
            (RowStep::PageDown, None) => page.saturating_sub(1),
            (RowStep::Up, Some(row)) => row.0.saturating_sub(1),
            (RowStep::Down, Some(row)) => row.0.saturating_add(1),
            (RowStep::PageUp, Some(row)) => row.0.saturating_sub(page),
            (RowStep::PageDown, Some(row)) => row.0.saturating_add(page),
        };
        Some(RowNo(to.min(last)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_moves_by_rows_and_pages_and_stays_inside_the_list() {
        // name, move, from, rows, page, the row after
        // name, move, from, rows, page, the row after
        type Case = (&'static str, RowStep, Option<u32>, u32, u32, Option<u32>);
        const CASES: &[Case] = &[
            ("down from nothing", RowStep::Down, None, 10, 4, Some(0)),
            ("up from nothing", RowStep::Up, None, 10, 4, Some(0)),
            (
                "page down from nothing",
                RowStep::PageDown,
                None,
                10,
                4,
                Some(3),
            ),
            ("end from nothing", RowStep::Bottom, None, 10, 4, Some(9)),
            ("down", RowStep::Down, Some(2), 10, 4, Some(3)),
            ("down at the end", RowStep::Down, Some(9), 10, 4, Some(9)),
            ("up", RowStep::Up, Some(2), 10, 4, Some(1)),
            ("up at the top", RowStep::Up, Some(0), 10, 4, Some(0)),
            ("page down", RowStep::PageDown, Some(2), 10, 4, Some(6)),
            (
                "page down past the end",
                RowStep::PageDown,
                Some(8),
                10,
                4,
                Some(9),
            ),
            ("page up", RowStep::PageUp, Some(6), 10, 4, Some(2)),
            (
                "page up past the top",
                RowStep::PageUp,
                Some(1),
                10,
                4,
                Some(0),
            ),
            ("home", RowStep::Top, Some(7), 10, 4, Some(0)),
            ("end", RowStep::Bottom, Some(2), 10, 4, Some(9)),
            (
                "a page of nothing is a row",
                RowStep::PageDown,
                Some(2),
                10,
                0,
                Some(3),
            ),
            ("no rows", RowStep::Down, None, 0, 4, None),
            (
                "a cursor past a shrunk list",
                RowStep::Down,
                Some(50),
                10,
                4,
                Some(9),
            ),
        ];
        for (name, step, from, rows, page, want) in CASES {
            assert_eq!(
                step.from(from.map(RowNo), *rows, *page),
                want.map(RowNo),
                "{name}"
            );
        }
    }
}
