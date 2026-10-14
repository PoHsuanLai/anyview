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

/// Whether `digits` is a run of digits with commas in the places a thousands separator goes: one
/// to three digits, then groups of exactly three.
fn grouped(digits: &str) -> bool {
    let mut groups = digits.split(',');
    let first = groups.next().unwrap_or("");
    (1..=3).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_digit())
        && groups.all(|group| group.len() == 3 && group.bytes().all(|b| b.is_ascii_digit()))
}

/// The number a cell holds, when it holds one: digits with an optional sign, thousands commas, a
/// point and an exponent, or a percentage. A word that Rust would read as a number (`inf`, `NaN`)
/// is not one.
fn number(cell: &str) -> Option<f64> {
    let text = cell.trim();
    let text = text.strip_suffix('%').map_or(text, str::trim_end);
    if text.contains(',') {
        let (sign, rest) = match text.strip_prefix(['-', '+']) {
            Some(rest) => (&text[..1], rest),
            None => ("", text),
        };
        let (whole, fraction) = rest.split_once('.').unwrap_or((rest, ""));
        if !grouped(whole) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let plain = format!("{sign}{}.{fraction}", whole.replace(',', ""));
        return plain.parse::<f64>().ok().filter(|value| value.is_finite());
    }
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

/// How many characters of a text cell decide where its row sorts. Cells that agree in their first
/// 64 characters (after trimming and lowercasing) keep the file's order between them.
const KEY_CHARS: usize = 64;

/// What a cell sorts by. Numbers come before text and blanks after both, whichever way the
/// column runs.
#[derive(Debug, PartialEq)]
enum Key {
    Number(f64),
    Text(Box<str>),
    Blank,
}

impl Key {
    fn of(cell: &str) -> Key {
        if let Some(value) = number(cell) {
            return Key::Number(value);
        }
        let text = cell.trim();
        if text.is_empty() {
            return Key::Blank;
        }
        Key::Text(
            text.chars()
                .take(KEY_CHARS)
                .flat_map(char::to_lowercase)
                .collect::<String>()
                .into_boxed_str(),
        )
    }

    const fn rank(&self) -> u8 {
        match self {
            Key::Number(_) => 0,
            Key::Text(_) => 1,
            Key::Blank => 2,
        }
    }
}

/// The digits at the front of `chars`, taken off it, without their leading zeros.
fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(d) = chars.next_if(char::is_ascii_digit) {
        run.push(d);
    }
    run.trim_start_matches('0').to_owned()
}

/// Two texts' order with the runs of digits in them compared as numbers, so `file2` comes before
/// `file10`. The texts are already case-folded.
fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (x, y) = (digits(&mut a), digits(&mut b));
                let order = x.len().cmp(&y.len()).then_with(|| x.cmp(&y));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Two keys' order for a column running `direction`: numbers, then text, then blanks, whichever
/// way it runs; only the order within numbers and within text turns around.
fn compare(a: &Key, b: &Key, direction: SortDirection) -> Ordering {
    let within = match (a, b) {
        (Key::Number(a), Key::Number(b)) => a.total_cmp(b),
        (Key::Text(a), Key::Text(b)) => natural(a, b),
        _ => Ordering::Equal,
    };
    a.rank().cmp(&b.rank()).then(match direction {
        SortDirection::Ascending => within,
        SortDirection::Descending => within.reverse(),
    })
}

/// The rows in the order a sort by one column gives: each entry is a row of the file, numbered
/// from 0 in the order `cells` come, which is the order of the column. Numbers (with thousands
/// commas, a sign or a percent sign, or kept as text) come first and compare as numbers, then
/// text in natural order without regard to case, then blanks; rows that compare equal stay in
/// the file's order.
///
/// The cells go by one at a time and only a key for each is kept (a number, or the start of the
/// text) with its row number, so the rows of a table of any size are never held to sort it.
pub fn sorted_rows<S: AsRef<str>>(
    direction: SortDirection,
    cells: impl IntoIterator<Item = S>,
) -> Vec<u32> {
    let mut keyed: Vec<(Key, u32)> = cells
        .into_iter()
        .zip(0..)
        .map(|(cell, row)| (Key::of(cell.as_ref()), row))
        .collect();
    keyed.sort_by(|a, b| compare(&a.0, &b.0, direction));
    keyed.into_iter().map(|(_, row)| row).collect()
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
            ("a leading plus", &["+4"], true),
            ("blanks are skipped", &["1", "", "  ", "4"], true),
            ("one word spoils it", &["1", "two", "3"], false),
            ("all blank is not numeric", &["", " "], false),
            ("no cells is not numeric", &[], false),
            ("rust's own words are not numbers", &["inf", "NaN"], false),
            (
                "a thousands comma is a number",
                &["1,000", "-2,500.5"],
                true,
            ),
            ("a misplaced comma is text", &["1,00"], false),
        ];
        for (name, cells, want) in CASES {
            assert_eq!(is_numeric(cells.iter().copied()), *want, "{name}");
        }
    }

    #[test]
    fn rows_sort_numbers_first_then_natural_text_then_blanks_with_ties_in_file_order() {
        use SortDirection::{Ascending, Descending};
        // name, the cells, direction, the rows after
        type Case = (
            &'static str,
            &'static [&'static str],
            SortDirection,
            &'static [u32],
        );
        const CASES: &[Case] = &[
            (
                "numbers ascending",
                &["10", "9", "100"],
                Ascending,
                &[1, 0, 2],
            ),
            (
                "numbers descending",
                &["10", "9", "100"],
                Descending,
                &[2, 0, 1],
            ),
            (
                "blank numbers stay last",
                &["", "2", "1"],
                Ascending,
                &[2, 1, 0],
            ),
            (
                "blank numbers stay last going down",
                &["", "2", "1"],
                Descending,
                &[1, 2, 0],
            ),
            (
                "negatives and signs",
                &["5", "-3", "+12", "0"],
                Ascending,
                &[1, 3, 0, 2],
            ),
            (
                "percentages",
                &["50%", "7 %", "100%"],
                Ascending,
                &[1, 0, 2],
            ),
            (
                "thousands separators",
                &["1,000", "999", "12,345.5"],
                Ascending,
                &[1, 0, 2],
            ),
            (
                "a misplaced comma is text, after the numbers",
                &["1,00", "5"],
                Ascending,
                &[1, 0],
            ),
            (
                "mixed puts numbers before text",
                &["b", "10", "a", "9"],
                Ascending,
                &[3, 1, 2, 0],
            ),
            (
                "mixed going down keeps numbers before text",
                &["b", "10", "a", "9", ""],
                Descending,
                &[1, 3, 0, 2, 4],
            ),
            ("text ignores case", &["b", "A", "c"], Ascending, &[1, 0, 2]),
            ("text descending", &["b", "A", "c"], Descending, &[2, 0, 1]),
            (
                "natural order reads digit runs as numbers",
                &["file10", "File2", "file1"],
                Ascending,
                &[2, 1, 0],
            ),
            (
                "leading zeros are no different",
                &["a007", "a7", "a06"],
                Ascending,
                &[2, 0, 1],
            ),
            (
                "ties keep the file's order",
                &["x", "x", "a"],
                Ascending,
                &[2, 0, 1],
            ),
            (
                "ties keep it going down too",
                &["x", "x", "a"],
                Descending,
                &[0, 1, 2],
            ),
            ("no rows", &[], Ascending, &[]),
        ];
        for (name, cells, direction, want) in CASES {
            let got = sorted_rows(*direction, cells.iter().copied());
            assert_eq!(&got, want, "{name}");
        }
    }

    #[test]
    fn text_that_agrees_in_its_first_characters_keeps_the_files_order() {
        let early = format!("{}b", "x".repeat(KEY_CHARS));
        let late = format!("{}a", "x".repeat(KEY_CHARS));
        let got = sorted_rows(SortDirection::Ascending, [&early, &late]);
        assert_eq!(got, [0, 1]);
    }
}
