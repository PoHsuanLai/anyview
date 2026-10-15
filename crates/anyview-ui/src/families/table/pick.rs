//! The rows picked in a sheet, read through the sort: where a new order puts them, and the text
//! Copy puts on the clipboard. The machine keeps the picked rows as places in the order shown
//! (`RowSelection`, a few ranges however many rows); the order itself is the view's, so these two
//! need it.

use super::doc::SheetDoc;
use anyview_text::RowIndex;
use ds_core::selection::RowSelection;

/// The row of the file shown at place `at`: through the sort's `order` when there is one.
fn file_row(order: Option<&[u32]>, at: usize) -> Option<u32> {
    match order {
        Some(order) => order.get(at).copied(),
        None => u32::try_from(at).ok(),
    }
}

/// Where each row of the file is shown under `order`, indexed by the file's row: the inverse of
/// the order, which lists the file's row for each place.
fn places(order: &[u32]) -> Vec<u32> {
    let mut places = vec![0; order.len()];
    for (place, file) in order.iter().enumerate() {
        if let (Some(slot), Ok(place)) = (places.get_mut(*file as usize), u32::try_from(place)) {
            // a u32 fits a usize
            *slot = place;
        }
    }
    places
}

/// The places `rows` (places under the order `from`, the file's own when `None`) are at under the
/// order `to`: the same rows of the file, picked after a sort. A selection of every one of `total`
/// rows is the same in any order and is not walked.
pub(super) fn reseat(
    rows: &RowSelection,
    total: usize,
    from: Option<&[u32]>,
    to: Option<&[u32]>,
) -> RowSelection {
    if total > 0 && rows.count() == total {
        return rows.clone();
    }
    let inverse = to.map(places);
    let moved = |at: usize| -> Option<usize> {
        let file = file_row(from, at)?;
        match &inverse {
            Some(inverse) => inverse.get(file as usize).map(|place| *place as usize), // a u32 fits a usize
            None => Some(file as usize), // a u32 fits a usize
        }
    };
    RowSelection::of_rows(
        rows.anchor().and_then(moved),
        rows.cursor().and_then(moved),
        rows.rows().filter_map(moved),
    )
}

/// A cell as a line of tab-separated text holds it: as it is, unless it has a tab, a line break
/// or a quote, when it is quoted with its quotes doubled (what a spreadsheet writes).
fn push_cell(text: &mut String, cell: &str) {
    if cell.contains(['\t', '\n', '\r', '"']) {
        text.push('"');
        text.push_str(&cell.replace('"', "\"\""));
        text.push('"');
    } else {
        text.push_str(cell);
    }
}

/// The rows `rows` of `sheet` as Numbers copies them: a line for each, its cells between tabs, in
/// the order shown (`order`, the file's own when `None`), with no header.
pub(super) fn copy_text(sheet: &SheetDoc, rows: &RowSelection, order: Option<&[u32]>) -> String {
    let columns = sheet.table.columns().0 as usize; // a u32 fits a usize
    let mut text = String::new();
    for (line, file) in rows.rows().filter_map(|at| file_row(order, at)).enumerate() {
        if line > 0 {
            text.push('\n');
        }
        for column in 0..columns {
            if column > 0 {
                text.push('\t');
            }
            push_cell(&mut text, &sheet.table.cell(RowIndex(file), column));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_text::{Coverage, HeaderMode, Table};

    fn sheet(rows: &[&[&str]]) -> SheetDoc {
        let rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
            .collect();
        SheetDoc {
            name: String::new(),
            table: Table::from_rows(rows, HeaderMode::Absent, Coverage::Whole),
            widths: Vec::new(),
            kinds: Vec::new(),
        }
    }

    #[test]
    fn picked_rows_are_copied_as_tab_separated_lines_in_the_order_shown() {
        let sheet = sheet(&[
            &["a", "1"],
            &["b", "two\tcolumns"],
            &["c", "say \"hi\""],
            &["d", "two\nlines"],
        ]);
        let order = [3, 2, 1, 0];
        // (name, rows picked by place, order, the text)
        let cases: Vec<(&str, RowSelection, Option<&[u32]>, &str)> = vec![
            ("one row", RowSelection::single(0), None, "a\t1"),
            (
                "rows apart",
                RowSelection::of_rows(None, None, [0, 2]),
                None,
                "a\t1\nc\t\"say \"\"hi\"\"\"",
            ),
            (
                "a tab and a line break are quoted",
                RowSelection::of_rows(None, None, [1, 3]),
                None,
                "b\t\"two\tcolumns\"\nd\t\"two\nlines\"",
            ),
            (
                "places go through the sort",
                RowSelection::of_rows(None, None, [0, 3]),
                Some(&order),
                "d\t\"two\nlines\"\na\t1",
            ),
            ("nothing picked", RowSelection::none(), None, ""),
        ];
        for (name, rows, order, want) in cases {
            assert_eq!(copy_text(&sheet, &rows, order), want, "{name}");
        }
    }

    #[test]
    fn a_sort_keeps_the_same_rows_of_the_file_picked() {
        let reversed = [3, 2, 1, 0];
        let rotated = [1, 2, 3, 0];
        // (name, picked by place, from, to, picked by place after, anchor, cursor)
        type Case<'a> = (
            &'a str,
            RowSelection,
            Option<&'a [u32]>,
            Option<&'a [u32]>,
            Vec<usize>,
            Option<usize>,
            Option<usize>,
        );
        let cases: Vec<Case> = vec![
            (
                "file order to reversed",
                RowSelection::of_rows(Some(0), Some(1), [0, 1]),
                None,
                Some(&reversed),
                vec![2, 3],
                Some(3),
                Some(2),
            ),
            (
                "reversed back to the file's own",
                RowSelection::of_rows(Some(3), Some(2), [2, 3]),
                Some(&reversed),
                None,
                vec![0, 1],
                Some(0),
                Some(1),
            ),
            (
                "one order to another",
                RowSelection::of_rows(Some(0), Some(0), [0]),
                Some(&reversed),
                Some(&rotated),
                vec![2],
                Some(2),
                Some(2),
            ),
            (
                "the same order",
                RowSelection::of_rows(Some(1), Some(2), [1, 2]),
                Some(&reversed),
                Some(&reversed),
                vec![1, 2],
                Some(1),
                Some(2),
            ),
            (
                "a place the order does not have goes",
                RowSelection::of_rows(Some(9), Some(9), [9]),
                Some(&reversed),
                None,
                vec![],
                None,
                None,
            ),
        ];
        for (name, rows, from, to, want, anchor, cursor) in cases {
            let got = reseat(&rows, 4, from, to);
            assert_eq!(got.rows().collect::<Vec<_>>(), want, "{name}");
            assert_eq!((got.anchor(), got.cursor()), (anchor, cursor), "{name}");
        }
    }

    #[test]
    fn every_row_picked_is_every_row_in_any_order() {
        let all = RowSelection::none().all(4);
        let reversed = [3, 2, 1, 0];
        assert_eq!(reseat(&all, 4, None, Some(&reversed)), all);
    }
}
