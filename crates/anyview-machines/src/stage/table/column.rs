//! A table's columns as a person reads them: which are numbers, and the order the rows take when
//! the view sorts by one. Sorting is a way of looking: it names rows of the file in another order
//! and never changes the file.

use std::cmp::Ordering;

/// Which way a sorted column runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortDirection {
    /// Smallest first, A before Z.
    Ascending,
    /// Largest first.
    Descending,
}

/// The column a table view is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnSort {
    /// The column, zero-based, among the file's own columns.
    pub column: u32,
    /// Which way it runs.
    pub direction: SortDirection,
}

impl ColumnSort {
    /// The sort after a press on `column`'s header: a column pressed again goes ascending, then
    /// descending, then back to the file's own order; any other column sorts ascending.
    pub fn after_press(current: Option<ColumnSort>, column: u32) -> Option<ColumnSort> {
        let direction = match current {
            Some(sort) if sort.column == column => match sort.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => return None,
            },
            Some(_) | None => SortDirection::Ascending,
        };
        Some(ColumnSort { column, direction })
    }
}

/// The number a cell holds, when it holds one: digits with an optional sign, a point and an
/// exponent, or a percentage. A word that Rust would read as a number (`inf`, `NaN`) is not one.
fn number(cell: &str) -> Option<f64> {
    let text = cell.trim();
    let text = text.strip_suffix('%').map_or(text, str::trim_end);
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// Whether a column is a column of numbers: it has at least one cell with something in it, and
/// every cell with something in it is a number. Blank cells do not count against it.
pub fn is_numeric<'a>(cells: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen = false;
    for cell in cells {
        if cell.trim().is_empty() {
            continue;
        }
        if number(cell).is_none() {
            return false;
        }
        seen = true;
    }
    seen
}

/// Two cells' order, blanks always last whichever way the column runs.
fn directed<T>(
    a: Option<&T>,
    b: Option<&T>,
    direction: SortDirection,
    compare: impl Fn(&T, &T) -> Ordering,
) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => match direction {
            SortDirection::Ascending => compare(a, b),
            SortDirection::Descending => compare(b, a),
        },
    }
}

/// How many characters of a text cell decide where its row sorts. Cells that agree in their first
/// 64 characters (after trimming and lowercasing) keep the file's order between them.
const KEY_CHARS: usize = 64;

/// The key a text cell sorts by, or `None` for a blank.
fn text_key(cell: &str) -> Option<Box<str>> {
    let text = cell.trim();
    (!text.is_empty()).then(|| {
        text.chars()
            .take(KEY_CHARS)
            .flat_map(char::to_lowercase)
            .collect::<String>()
            .into_boxed_str()
    })
}

/// The rows in the order a sort by one column gives: each entry is a row of the file, numbered
/// from 0 in the order `cells` come, which is the order of the column. A numeric column compares
/// as numbers, any other as text without regard to case; rows that compare equal stay in the
/// file's order.
///
/// The cells go by one at a time and only a key for each is kept (a number, or the start of the
/// text) with its row number, so the rows of a table of any size are never held to sort it.
pub fn sorted_rows<S: AsRef<str>>(
    direction: SortDirection,
    numeric: bool,
    cells: impl IntoIterator<Item = S>,
) -> Vec<u32> {
    if numeric {
        let mut keyed: Vec<(Option<f64>, u32)> = cells
            .into_iter()
            .zip(0..)
            .map(|(cell, row)| (number(cell.as_ref()), row))
            .collect();
        keyed.sort_by(|a, b| directed(a.0.as_ref(), b.0.as_ref(), direction, f64::total_cmp));
        keyed.into_iter().map(|(_, row)| row).collect()
    } else {
        let mut keyed: Vec<(Option<Box<str>>, u32)> = cells
            .into_iter()
            .zip(0..)
            .map(|(cell, row)| (text_key(cell.as_ref()), row))
            .collect();
        keyed.sort_by(|a, b| {
            directed(
                a.0.as_ref(),
                b.0.as_ref(),
                direction,
                <Box<str> as Ord>::cmp,
            )
        });
        keyed.into_iter().map(|(_, row)| row).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn by(column: u32, direction: SortDirection) -> Option<ColumnSort> {
        Some(ColumnSort { column, direction })
    }

    #[test]
    fn a_header_press_cycles_ascending_descending_and_the_files_own_order() {
        use SortDirection::{Ascending, Descending};
        // name, the sort now, the column pressed, the sort after
        type Case = (&'static str, Option<ColumnSort>, u32, Option<ColumnSort>);
        const CASES: &[Case] = &[
            ("nothing sorted", None, 1, by(1, Ascending)),
            (
                "ascending goes descending",
                by(1, Ascending),
                1,
                by(1, Descending),
            ),
            (
                "descending goes back to the file",
                by(1, Descending),
                1,
                None,
            ),
            (
                "another column starts ascending",
                by(1, Descending),
                2,
                by(2, Ascending),
            ),
        ];
        for (name, now, pressed, after) in CASES {
            assert_eq!(ColumnSort::after_press(*now, *pressed), *after, "{name}");
        }
    }

    #[test]
    fn a_column_is_numeric_when_every_filled_cell_is_a_number() {
        // name, the cells, whether the column is numeric
        type Case = (&'static str, &'static [&'static str], bool);
        const CASES: &[Case] = &[
            ("integers", &["1", "20", "-3"], true),
            ("decimals and exponents", &["1.5", " 2e3 ", "-.5"], true),
            ("percentages", &["12%", "7.5 %"], true),
            ("blanks are skipped", &["1", "", "  ", "4"], true),
            ("one word spoils it", &["1", "two", "3"], false),
            ("all blank is not numeric", &["", " "], false),
            ("no cells is not numeric", &[], false),
            ("rust's own words are not numbers", &["inf", "NaN"], false),
            ("a thousands comma is text", &["1,000"], false),
        ];
        for (name, cells, want) in CASES {
            assert_eq!(is_numeric(cells.iter().copied()), *want, "{name}");
        }
    }

    #[test]
    fn rows_sort_by_number_or_text_with_blanks_last_and_ties_in_file_order() {
        use SortDirection::{Ascending, Descending};
        // name, the cells, numeric, direction, the rows after
        type Case = (
            &'static str,
            &'static [&'static str],
            bool,
            SortDirection,
            &'static [u32],
        );
        const CASES: &[Case] = &[
            (
                "numbers ascending",
                &["10", "9", "100"],
                true,
                Ascending,
                &[1, 0, 2],
            ),
            (
                "numbers descending",
                &["10", "9", "100"],
                true,
                Descending,
                &[2, 0, 1],
            ),
            (
                "blank numbers stay last",
                &["", "2", "1"],
                true,
                Ascending,
                &[2, 1, 0],
            ),
            (
                "blank numbers stay last going down",
                &["", "2", "1"],
                true,
                Descending,
                &[1, 2, 0],
            ),
            (
                "text ignores case",
                &["b", "A", "c"],
                false,
                Ascending,
                &[1, 0, 2],
            ),
            (
                "text descending",
                &["b", "A", "c"],
                false,
                Descending,
                &[2, 0, 1],
            ),
            (
                "ties keep the file's order",
                &["x", "x", "a"],
                false,
                Ascending,
                &[2, 0, 1],
            ),
            (
                "ties keep it going down too",
                &["x", "x", "a"],
                false,
                Descending,
                &[0, 1, 2],
            ),
            ("no rows", &[], false, Ascending, &[]),
        ];
        for (name, cells, numeric, direction, want) in CASES {
            let got = sorted_rows(*direction, *numeric, cells.iter().copied());
            assert_eq!(&got, want, "{name}");
        }
    }

    #[test]
    fn text_that_agrees_in_its_first_characters_keeps_the_files_order() {
        let early = format!("{}b", "x".repeat(KEY_CHARS));
        let late = format!("{}a", "x".repeat(KEY_CHARS));
        let got = sorted_rows(SortDirection::Ascending, false, [&early, &late]);
        assert_eq!(got, [0, 1]);
    }
}
