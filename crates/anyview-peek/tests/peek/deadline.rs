//! A peek whose time is already up ends with the start it had, or with the facts card: never with
//! a fault, and never by reading on.

use crate::support::{self, Home};
use anyview_core::{Input, PeekBudget, Sniffed};
use anyview_peek::{Body, Unavailable, peek};
use anyview_text::Tally;
use std::time::Duration;

/// A budget with room for every file here and no time at all.
fn late() -> PeekBudget {
    support::pane_budget().with_time(Duration::ZERO)
}

/// What a peek that is late comes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ends {
    /// The facts card: nothing was ready in time.
    Slow,
    /// The start of the text, counted as a lower bound.
    Start,
}

fn text_file(dir: &std::path::Path, name: &str, bytes: &[u8]) -> (Input, Sniffed) {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    support::on_disk(&path, 0)
}

#[test]
fn a_peek_whose_time_is_up_ends_with_its_start_or_the_facts_card() {
    let dir = tempfile::tempdir().unwrap();
    // Three mebibytes of lines: more than the first chunk a late peek still reads.
    let long = "a line of text\n".repeat(3 * 1024 * 1024 / 15);
    let rows = "a,b\n1,2\n".repeat(1000);
    // name, the file, how the peek ends
    let cases: Vec<(&str, (Input, Sniffed), Ends)> = vec![
        (
            "a zip",
            support::zip_on_disk(dir.path(), &[("a.txt", "x"), ("b.txt", "y")]),
            Ends::Slow,
        ),
        (
            "a font",
            support::fixture(Home::Font, "blocks.ttf"),
            Ends::Slow,
        ),
        (
            "a CSV",
            text_file(dir.path(), "rows.csv", rows.as_bytes()),
            Ends::Slow,
        ),
        (
            "a Markdown file",
            text_file(dir.path(), "notes.md", b"# Title\n\nbody\n"),
            Ends::Slow,
        ),
        (
            "a JSON file",
            text_file(dir.path(), "data.json", b"{\"a\":[1,2,3]}"),
            Ends::Slow,
        ),
        (
            "a long text file",
            text_file(dir.path(), "long.txt", long.as_bytes()),
            Ends::Start,
        ),
    ];
    for (name, (src, sniffed), ends) in cases {
        let peeked = peek(&src, &sniffed, &late());
        let how = match &peeked.body {
            Body::Unavailable(Unavailable::Slow) => Ends::Slow,
            Body::Plain(plain) if matches!(plain.total, Tally::AtLeast(_)) => Ends::Start,
            other @ (Body::Picture(_)
            | Body::Page(_)
            | Body::Plain(_)
            | Body::Code(_)
            | Body::Markdown(_)
            | Body::Table(_)
            | Body::Tree(_)
            | Body::Archive(_)
            | Body::Font(_)
            | Body::Folder(_)
            | Body::FactsOnly(_)
            | Body::Unavailable(_)) => panic!("{name}: {other:?}"),
        };
        assert_eq!(how, ends, "{name}");
    }
}
