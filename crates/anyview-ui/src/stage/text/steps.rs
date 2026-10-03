//! Where a key step through the lines lands: pure arithmetic over the line at the top, the
//! length of the file and the size of a page.

use super::model::{TextExtent, TextStep};
use anyview_core::LineIndex;

/// The line at the top after `step` from `line`. A step towards the end stops where the last
/// page is at the bottom of the view, and never moves backwards from a line beyond it (the wheel
/// can leave the reader past it); a step towards the start stops at the first line.
pub(super) fn stepped(line: LineIndex, step: TextStep, extent: TextExtent) -> LineIndex {
    let page = extent.page.0.max(1);
    let last_page = extent.lines.0.saturating_sub(page);
    let down = |by: u32| line.0.saturating_add(by).min(last_page.max(line.0));
    LineIndex(match step {
        TextStep::LineUp => line.0.saturating_sub(1),
        TextStep::PageUp => line.0.saturating_sub(page),
        TextStep::LineDown => down(1),
        TextStep::PageDown => down(page),
        TextStep::Top => 0,
        TextStep::Bottom => last_page,
    })
}

#[cfg(test)]
mod tests {
    use super::super::model::{LineTotal, PageLines};
    use super::*;

    const fn extent(lines: u32, page: u32) -> TextExtent {
        TextExtent {
            lines: LineTotal(lines),
            page: PageLines(page),
        }
    }

    #[test]
    fn a_key_step_lands_where_the_table_says() {
        // name, line now, step, lines in the file, lines in a page, line after
        const CASES: &[(&str, u32, TextStep, u32, u32, u32)] = &[
            ("a line down", 10, TextStep::LineDown, 100, 20, 11),
            ("a line up", 10, TextStep::LineUp, 100, 20, 9),
            (
                "up stops at the first line",
                0,
                TextStep::LineUp,
                100,
                20,
                0,
            ),
            ("a page down", 10, TextStep::PageDown, 100, 20, 30),
            ("a page up", 30, TextStep::PageUp, 100, 20, 10),
            (
                "a page up near the top stops at it",
                5,
                TextStep::PageUp,
                100,
                20,
                0,
            ),
            (
                "down stops with the end at the bottom",
                75,
                TextStep::PageDown,
                100,
                20,
                80,
            ),
            (
                "a line down at the last page stays",
                80,
                TextStep::LineDown,
                100,
                20,
                80,
            ),
            (
                "beyond the last page does not move back",
                95,
                TextStep::PageDown,
                100,
                20,
                95,
            ),
            ("home", 55, TextStep::Top, 100, 20, 0),
            ("end", 3, TextStep::Bottom, 100, 20, 80),
            (
                "a file shorter than a page stays at the top",
                0,
                TextStep::PageDown,
                8,
                20,
                0,
            ),
            ("end of a short file", 0, TextStep::Bottom, 8, 20, 0),
            (
                "a page of nothing still moves one line",
                4,
                TextStep::PageDown,
                100,
                0,
                5,
            ),
            ("an empty file", 0, TextStep::LineDown, 0, 20, 0),
        ];
        for (name, line, step, lines, page, want) in CASES {
            assert_eq!(
                stepped(LineIndex(*line), *step, extent(*lines, *page)),
                LineIndex(*want),
                "{name}"
            );
        }
    }
}
