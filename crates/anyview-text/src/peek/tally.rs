//! Counts a peek reports: exact when it saw the whole file, a lower bound when it saw the start.

use crate::encoding::Coverage;

/// How many lines or rows a file has, as far as a peek knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tally {
    /// The whole file was read.
    Exact(u32),
    /// Only the start was read: there are at least this many.
    AtLeast(u32),
}

impl Tally {
    /// `count`, exact or a lower bound by how much of the file was read.
    pub(super) fn of(count: usize, coverage: Coverage) -> Self {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        match coverage {
            Coverage::Whole => Tally::Exact(count),
            Coverage::Prefix => Tally::AtLeast(count),
        }
    }

    /// `1,234`, or `1,234+` for a lower bound.
    pub fn text(self) -> String {
        match self {
            Tally::Exact(count) => grouped(count),
            Tally::AtLeast(count) => format!("{}+", grouped(count)),
        }
    }
}

/// `count` with a comma between each group of three digits.
pub(super) fn grouped(count: u32) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tallies_read_with_separators_and_a_plus_for_a_lower_bound() {
        // name, tally, text
        const CASES: &[(&str, Tally, &str)] = &[
            ("zero", Tally::Exact(0), "0"),
            ("small", Tally::Exact(999), "999"),
            ("thousand", Tally::Exact(1_000), "1,000"),
            ("million", Tally::Exact(1_234_567), "1,234,567"),
            ("lower bound", Tally::AtLeast(1_500), "1,500+"),
            ("largest", Tally::Exact(u32::MAX), "4,294,967,295"),
        ];
        for (name, tally, text) in CASES {
            assert_eq!(tally.text(), *text, "{name}");
        }
    }

    #[test]
    fn a_count_is_exact_only_for_a_whole_file() {
        assert_eq!(Tally::of(7, Coverage::Whole), Tally::Exact(7));
        assert_eq!(Tally::of(7, Coverage::Prefix), Tally::AtLeast(7));
    }
}
