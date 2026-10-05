//! What separates a table's cells. The file's name says comma or tab; the rows say more: a
//! `.csv` written with semicolons (a locale that writes decimals with a comma) or pipes is read
//! with the character that splits its first rows into the same number of cells.

use anyview_core::Delimiter;
use ds_core::word::Word;

/// The rows looked at.
const SAMPLE_ROWS: usize = 20;

/// A character that separates cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Separator {
    /// `,`
    Comma,
    /// A tab.
    Tab,
    /// `;`
    Semicolon,
    /// `|`
    Pipe,
}

impl Separator {
    /// The byte the character is.
    pub(crate) fn byte(self) -> u8 {
        match self {
            Separator::Comma => b',',
            Separator::Tab => b'\t',
            Separator::Semicolon => b';',
            Separator::Pipe => b'|',
        }
    }

    /// The separator a file's name declares.
    pub(crate) fn declared(delimiter: Delimiter) -> Self {
        match delimiter {
            Delimiter::Comma => Separator::Comma,
            Delimiter::Tab => Separator::Tab,
        }
    }
}

/// How well `separator` explains `text`: the number of sampled rows that have the most common
/// cell count, when that count is more than one cell. Zero when it splits nothing.
fn score(text: &str, separator: Separator) -> usize {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(separator.byte())
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let widths: Vec<usize> = reader
        .records()
        .take(SAMPLE_ROWS)
        .filter_map(Result::ok)
        .map(|record| record.len())
        .collect();
    let best = widths
        .iter()
        .filter(|width| **width > 1)
        .map(|width| widths.iter().filter(|other| *other == width).count())
        .max();
    best.unwrap_or(0)
}

/// The separator `text` is written with: the declared one unless another splits the sample into
/// strictly more rows of one width.
pub(crate) fn sniff(text: &str, declared: Delimiter) -> Separator {
    let declared = Separator::declared(declared);
    let sample = &text[..text
        .char_indices()
        .map(|(at, _)| at)
        .take_while(|at| *at <= 16 * 1024)
        .last()
        .unwrap_or(0)];
    let mut best = (declared, score(sample, declared));
    for candidate in Separator::ALL {
        let candidate_score = score(sample, *candidate);
        if candidate_score > best.1 {
            best = (*candidate, candidate_score);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_choose_the_separator_and_the_name_wins_a_tie() {
        // name, declared, text, wanted
        const CASES: &[(&str, Delimiter, &str, Separator)] = &[
            (
                "commas",
                Delimiter::Comma,
                "a,b\n1,2\n3,4\n",
                Separator::Comma,
            ),
            ("tabs", Delimiter::Tab, "a\tb\n1\t2\n", Separator::Tab),
            (
                "semicolons in a csv",
                Delimiter::Comma,
                "a;b;c\n1,5;2;3\n4;5;6\n",
                Separator::Semicolon,
            ),
            (
                "pipes in a csv",
                Delimiter::Comma,
                "a|b\n1|2\n3|4\n",
                Separator::Pipe,
            ),
            (
                "tabs in a csv",
                Delimiter::Comma,
                "a\tb\n1\t2\n",
                Separator::Tab,
            ),
            (
                "a quoted comma does not split",
                Delimiter::Comma,
                "a;b\n\"x,y\";2\n\"z,w\";3\n",
                Separator::Semicolon,
            ),
            (
                "one column keeps the name",
                Delimiter::Comma,
                "a\nb\nc\n",
                Separator::Comma,
            ),
            ("empty keeps the name", Delimiter::Tab, "", Separator::Tab),
            (
                "a tie keeps the name",
                Delimiter::Comma,
                "a,b;c\n1,2;3\n",
                Separator::Comma,
            ),
        ];
        for (name, declared, text, want) in CASES {
            assert_eq!(sniff(text, *declared), *want, "{name}");
        }
    }
}
