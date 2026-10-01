//! Does a table's first row name its columns? A guess from what the rows below look like, since
//! a CSV file does not say.

/// Rows below the first that are looked at.
const SAMPLE_ROWS: usize = 20;

/// The longest cell that still reads as a column name.
const NAME_MAX_CHARS: usize = 32;

/// Whether `text` is a number: an optional sign, digits with at most one point, and an optional
/// exponent. Words such as `inf` and `nan` are text.
fn is_number(text: &str) -> bool {
    let text = text.trim();
    text.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
        && text.parse::<f64>().is_ok()
}

/// The cells of column `column` in `rows` that are not empty.
fn column_below(rows: &[Vec<String>], column: usize) -> Vec<&str> {
    rows.iter()
        .take(SAMPLE_ROWS)
        .filter_map(|row| row.get(column))
        .map(String::as_str)
        .filter(|cell| !cell.trim().is_empty())
        .collect()
}

/// A vote for or against the first row being a header, from one column.
fn vote(name: &str, below: &[&str]) -> i32 {
    let numbers_below = !below.is_empty() && below.iter().all(|cell| is_number(cell));
    match (name.trim().is_empty(), is_number(name), numbers_below) {
        (true, _, _) | (false, true, _) => -1,
        (false, false, true) => 1,
        (false, false, false) => 0,
    }
}

/// Whether `first` looks like a header for `below`. A header has no number or empty cell, no
/// repeated name, and names columns whose data below is numeric; when the data gives no hint,
/// short distinct words that do not recur below count as names.
pub(super) fn looks_like_header(first: &[String], below: &[Vec<String>]) -> bool {
    if first.is_empty() || below.is_empty() {
        return false;
    }
    let mut seen: Vec<String> = Vec::new();
    for name in first {
        let lower = name.trim().to_lowercase();
        if !lower.is_empty() && seen.contains(&lower) {
            return false;
        }
        seen.push(lower);
    }
    let score: i32 = first
        .iter()
        .enumerate()
        .map(|(column, name)| vote(name, &column_below(below, column)))
        .sum();
    match score {
        1.. => true,
        0 => first.iter().enumerate().all(|(column, name)| {
            let name = name.trim();
            !name.is_empty()
                && !is_number(name)
                && name.chars().count() <= NAME_MAX_CHARS
                && !column_below(below, column).contains(&name)
        }),
        ..=-1 => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|line| line.split(',').map(str::to_owned).collect())
            .collect()
    }

    #[test]
    fn the_first_row_is_a_header_when_the_rows_below_say_so() {
        // name, csv, is a header
        const CASES: &[(&str, &str, bool)] = &[
            (
                "a numeric column under a word",
                "name,age\nAnn,30\nBob,25",
                true,
            ),
            (
                "an id column and a text column",
                "id,name\n1,Ann\n2,Bob",
                true,
            ),
            (
                "dates and numbers",
                "Date,Value\n2024-01-01,3.5\n2024-01-02,4",
                true,
            ),
            ("an unnamed index column", ",a,b\n1,2,3", true),
            ("data in the first row", "Ann,30\nBob,25", false),
            ("all numbers", "1,2,3\n4,5,6", false),
            ("a repeated name", "a,a\nx,y", false),
            ("one row only", "name,age", false),
            (
                "text only, distinct and short",
                "name,city\nAnn,Oslo\nBob,Rome",
                true,
            ),
            (
                "text only, the first row recurs below",
                "Ann,Oslo\nAnn,Rome",
                false,
            ),
            ("an empty cell and no hint", "a,\nb,c", false),
            ("words that are not numbers", "inf,nan\n1,2", true),
        ];
        for (name, csv, want) in CASES {
            let table = rows(csv);
            assert_eq!(looks_like_header(&table[0], &table[1..]), *want, "{name}");
        }
    }

    #[test]
    fn numbers_are_signed_decimals_with_an_optional_exponent() {
        const CASES: &[(&str, bool)] = &[
            ("42", true),
            ("-3.5", true),
            ("+7", true),
            (".5", true),
            ("1e9", true),
            (" 12 ", true),
            ("inf", false),
            ("nan", false),
            ("12abc", false),
            ("", false),
            ("1,000", false),
        ];
        for (text, want) in CASES {
            assert_eq!(is_number(text), *want, "{text:?}");
        }
    }
}
