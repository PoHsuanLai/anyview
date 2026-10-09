//! A peek of a large text file holds a small multiple of the bytes it read, not a multiple of the
//! rows or values in them. This file holds one test, which measures the process's resident memory,
//! so nothing else may run beside it.
//!
//! Its own binary, in a process of its own: it resets the kernel's high-water mark of the process's memory, so no other test may run beside it.

#![allow(clippy::unwrap_used)]

#[path = "text/support/mod.rs"]
mod support;

use anyview_core::{Peek, PeekBudget};
use anyview_fs::OnDisk;
use anyview_text::{PlainPeek, TablePeek, TreePeek};
use support::{budget, written};

/// Resident memory in KiB: the current size and the high-water mark since it was last reset.
fn resident() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let field = |name: &str| -> u64 {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap()
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// The most resident bytes `run` adds to what the process held when it started: the kernel's
/// high-water mark is reset first (writing 5 to `clear_refs`), so what ran before does not count.
fn peak_of(run: impl FnOnce()) -> u64 {
    std::fs::write("/proc/self/clear_refs", "5").unwrap();
    let (before, _) = resident();
    run();
    let (_, most) = resident();
    most.saturating_sub(before) * 1024
}

const FILE_BYTES: usize = 16 * 1024 * 1024;

/// How many times the file's size a peek may hold at its peak: the bytes read, the text decoded
/// from them, and a little more.
const MULTIPLE: u64 = 4;

fn budget_of(bytes: usize) -> PeekBudget {
    budget(bytes as u64)
}

#[test]
fn peeks_of_files_made_of_one_byte_rows_hold_a_small_multiple_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    // name, file name, one repeating unit
    let cases: [(&str, &str, &str); 4] = [
        ("empty lines", "nl.txt", "\n"),
        ("one-cell rows", "rows.csv", "a\n"),
        ("json lines", "values.jsonl", "1\n"),
        ("one big json array", "array.json", "1,"),
    ];
    for (name, file, unit) in cases {
        let mut text = unit.repeat(FILE_BYTES / unit.len());
        if file == "array.json" {
            text = format!("[{text}1]");
        }
        let (src, sniffed) = written(dir.path(), file, text.as_bytes());
        drop(text);
        let budget = budget_of(FILE_BYTES + 16);
        let peak = peak_of(|| match file {
            "nl.txt" => {
                let peeked = PlainPeek::peek(&src.on_disk(), &sniffed, &budget).unwrap();
                assert_eq!(peeked.lines.len(), 40, "{name}");
            }
            "rows.csv" => {
                let peeked = TablePeek::peek(&src.on_disk(), &sniffed, &budget).unwrap();
                assert_eq!(peeked.rows.len(), 40, "{name}");
            }
            "values.jsonl" => {
                let peeked = TreePeek::peek(&src.on_disk(), &sniffed, &budget).unwrap();
                assert_eq!(peeked.top.len(), 40, "{name}");
            }
            _ => {
                // Over the size a document is parsed at: refused, and refused cheaply.
                assert!(
                    TreePeek::peek(&src.on_disk(), &sniffed, &budget).is_err(),
                    "{name}"
                );
            }
        });
        assert!(
            peak <= MULTIPLE * FILE_BYTES as u64,
            "{name}: {peak} resident bytes at the peak for a file of {FILE_BYTES}"
        );
    }
}
