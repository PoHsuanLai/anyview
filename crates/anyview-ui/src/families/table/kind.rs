//! What a column holds, as the eye reads it: numbers, yes/no, links, tags or plain text. A column's
//! kind is decided once per sheet from the rows its sample holds and then only chooses how each
//! cell is drawn; the file's text is never changed. Whether a cell is a number is the machines'
//! `is_numeric`, the one place that knows (the sort reads it too).

use crate::is_numeric;

/// How a column's cells are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnKind {
    /// Anything else: drawn as written.
    #[default]
    Text,
    /// Every cell is a number: set at the trailing edge in tabular figures, digits as written.
    Number,
    /// Every cell is a yes or a no: a tick for yes and nothing for no.
    YesNo,
    /// Every cell is a web address, a bare domain or an email address: quiet text, underlined
    /// faintly.
    Link,
    /// A short value from a small set that many rows repeat: drawn in a small rounded tag.
    Tag,
}

/// Fewest filled cells a sample needs before a column of text can be a column of tags: a short
/// column with few distinct values is just a few values.
const TAG_ROWS: usize = 20;
/// Most distinct values a column of tags has.
const TAG_VALUES: usize = 12;
/// Longest value a tag holds, in characters.
const TAG_CHARS: usize = 24;
/// Each tag value is repeated at least this often on average (filled cells per distinct value).
const TAG_REPEATS: usize = 2;

/// The words that read as a yes.
const YES: [&str; 5] = ["true", "yes", "y", "✓", "✔"];
/// The words that read as a no.
const NO: [&str; 5] = ["false", "no", "n", "✗", "✘"];

/// Whether `cell` reads as a yes (a tick is drawn for it).
pub(super) fn is_yes(cell: &str) -> bool {
    let text = cell.trim().to_lowercase();
    text == "1" || YES.contains(&text.as_str())
}

/// Whether `cell` reads as a no.
fn is_no(cell: &str) -> bool {
    let text = cell.trim().to_lowercase();
    text == "0" || NO.contains(&text.as_str())
}

/// The kind of the column whose cells are `cells`; empty cells are skipped. A column with nothing
/// in it is text.
///
/// Yes/no is the most careful: `1` and `0` count only beside a word (`true`, `yes`, a tick), so a
/// plain column of ones and zeros stays a column of numbers. Tags need at least [`TAG_ROWS`] filled
/// cells, at most [`TAG_VALUES`] distinct values, each of at most [`TAG_CHARS`] characters, and
/// [`TAG_REPEATS`] cells per distinct value.
pub(super) fn column_kind<'a>(cells: impl IntoIterator<Item = &'a str>) -> ColumnKind {
    let filled: Vec<&str> = cells
        .into_iter()
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .collect();
    if filled.is_empty() {
        return ColumnKind::Text;
    }
    if is_numeric(filled.iter().copied()) {
        return ColumnKind::Number;
    }
    let answers = filled.iter().all(|cell| is_yes(cell) || is_no(cell));
    let worded = filled.iter().any(|cell| *cell != "1" && *cell != "0");
    if answers && worded {
        return ColumnKind::YesNo;
    }
    if filled.iter().all(|cell| is_link(cell)) {
        return ColumnKind::Link;
    }
    let mut values: Vec<&str> = filled.clone();
    values.sort_unstable();
    values.dedup();
    let short = values
        .iter()
        .all(|value| value.chars().count() <= TAG_CHARS);
    if short
        && filled.len() >= TAG_ROWS
        && values.len() <= TAG_VALUES
        && filled.len() >= values.len() * TAG_REPEATS
    {
        return ColumnKind::Tag;
    }
    ColumnKind::Text
}

/// The top-level domains a bare domain (no `http://`, no `www.`) may end in; two letters of a
/// country also pass unless they are a file's extension (`notes.md`, `main.rs`).
const DOMAINS: [&str; 12] = [
    "com", "org", "net", "edu", "gov", "io", "dev", "app", "info", "biz", "ai", "co",
];
/// Two-letter endings that are more often a file's extension than a country.
const EXTENSIONS: [&str; 14] = [
    "py", "rs", "sh", "md", "pl", "js", "ts", "rb", "cs", "gz", "xz", "pm", "sc", "ps",
];

/// Whether `text` is a web address, a bare domain with a top-level domain, or an email address.
fn is_link(text: &str) -> bool {
    if text.chars().any(char::is_whitespace) {
        return false;
    }
    for scheme in ["https://", "http://"] {
        let Some(head) = text.get(..scheme.len()) else {
            continue;
        };
        if head.eq_ignore_ascii_case(scheme) {
            return is_host(host_of(&text[scheme.len()..]), true);
        }
    }
    if let Some((local, host)) = text.split_once('@') {
        return !local.is_empty() && !host.contains('@') && is_host(host, true);
    }
    let hosted = host_of(text);
    let www = hosted
        .get(..4)
        .is_some_and(|head| head.eq_ignore_ascii_case("www."));
    let path = hosted.len() < text.len();
    is_host(hosted, www || path)
}

/// The host in `text`: up to the first `/`, `?` or `#`.
fn host_of(text: &str) -> &str {
    text.split(['/', '?', '#']).next().unwrap_or("")
}

/// Whether `host` is dotted labels of letters, digits and hyphens ending in a top-level domain of
/// letters. Any top-level domain passes when `any` is set (a scheme, `www.`, a path or an email
/// vouches for it); otherwise it must be one of [`DOMAINS`] or a country's two letters.
fn is_host(host: &str, any: bool) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let [.., last] = labels.as_slice() else {
        return false;
    };
    let named = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        });
    let tld = last.to_lowercase();
    named
        && tld.chars().count() >= 2
        && tld.chars().all(char::is_alphabetic)
        && (any
            || DOMAINS.contains(&tld.as_str())
            || (tld.chars().count() == 2 && tld.is_ascii() && !EXTENSIONS.contains(&tld.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(cells: &[&str]) -> ColumnKind {
        column_kind(cells.iter().copied())
    }

    fn repeated(values: &[&str], times: usize) -> Vec<String> {
        (0..times)
            .flat_map(|_| values.iter().map(|v| (*v).to_owned()))
            .collect()
    }

    #[test]
    fn numbers_with_separators_and_signs_are_numbers_and_blanks_do_not_count() {
        assert_eq!(kind(&["1,240", "-3.5", "", "+7"]), ColumnKind::Number);
        assert_eq!(kind(&["1", "x"]), ColumnKind::Text);
        assert_eq!(kind(&["", " "]), ColumnKind::Text);
    }

    #[test]
    fn yes_and_no_words_are_yes_no_but_a_plain_zero_one_column_stays_numbers() {
        assert_eq!(kind(&["true", "FALSE", "", "True"]), ColumnKind::YesNo);
        assert_eq!(kind(&["yes", "no", "Y", "n"]), ColumnKind::YesNo);
        assert_eq!(kind(&["✓", "✗", "✓"]), ColumnKind::YesNo);
        assert_eq!(kind(&["true", "0", "1"]), ColumnKind::YesNo);
        assert_eq!(kind(&["1", "0", "1", "0"]), ColumnKind::Number);
        assert_eq!(kind(&["yes", "maybe"]), ColumnKind::Text);
        assert_eq!(kind(&["true", "2"]), ColumnKind::Text);
    }

    #[test]
    fn addresses_domains_and_emails_are_links_but_file_names_are_not() {
        assert_eq!(
            kind(&["https://a.com/x?y=1", "http://b.org", "www.c.net"]),
            ColumnKind::Link
        );
        assert_eq!(kind(&["nyc.gov", "paris.fr", "x.org/a"]), ColumnKind::Link);
        assert_eq!(kind(&["a@b.com", "me@x.co.uk"]), ColumnKind::Link);
        assert_eq!(kind(&["notes.md", "main.rs"]), ColumnKind::Text);
        assert_eq!(kind(&["report.docx", "a.txt"]), ColumnKind::Text);
        assert_eq!(kind(&["https://a.com", "plain words"]), ColumnKind::Text);
        assert_eq!(kind(&["3.14", "2.71"]), ColumnKind::Number);
    }

    #[test]
    fn a_short_set_repeated_over_many_rows_is_tags_and_anything_else_is_text() {
        let tags = repeated(&["Asia", "Europe", "Africa"], 10);
        assert_eq!(
            column_kind(tags.iter().map(String::as_str)),
            ColumnKind::Tag
        );
        let few = repeated(&["Asia", "Europe"], 5);
        assert_eq!(
            column_kind(few.iter().map(String::as_str)),
            ColumnKind::Text,
            "ten rows are too few to call a set"
        );
        let unique: Vec<String> = (0..40).map(|n| format!("name {n}")).collect();
        assert_eq!(
            column_kind(unique.iter().map(String::as_str)),
            ColumnKind::Text
        );
        let long = repeated(&["a value that runs well past twenty-four characters"], 30);
        assert_eq!(
            column_kind(long.iter().map(String::as_str)),
            ColumnKind::Text
        );
    }
}
