//! A file that did not open, or went away while it was open, as a person sees it under the
//! harness: what is wrong in words, and what can be done about it.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FilePath;
use anyview_ui::HostRequest;
use ds::prelude::Appearance;
use ds_harness::{Driver, Input, Query};
use std::io::Write;
use support::{Wiring, settle, text_file, wired};

#[test]
fn a_file_deleted_while_it_is_open_says_so_instead_of_showing_the_stale_copy() {
    let dir = tempfile::tempdir().unwrap();
    let file = text_file(dir.path(), "live.txt", "line", 5);
    let (mut harness, _, edge) = wired(
        std::slice::from_ref(&file),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    assert!(
        harness.count(".viewer-text") == 1,
        "the text shows to begin with"
    );
    std::fs::remove_file(&file).unwrap();
    edge.changed(FilePath::new(&file).unwrap());
    settle(&mut harness);
    let said = harness.text_of(".viewer-failed").unwrap_or_default();
    assert!(said.contains("moved or deleted"), "{said:?}");
    assert_eq!(harness.count(".viewer-text"), 0, "the stale lines are gone");
    assert_eq!(
        harness.count(".viewer-failed .ds-button"),
        0,
        "a file that is not there has nothing to open with"
    );
}

#[test]
fn a_valid_json_over_the_limit_says_it_is_too_large_and_offers_the_way_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.json");
    // A valid array just past the 64 MiB the tree reads.
    let mut big = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    big.write_all(b"[").unwrap();
    let chunk = [b"0,".repeat(512 * 1024)].concat();
    for _ in 0..65 {
        big.write_all(&chunk).unwrap();
    }
    big.write_all(b"0]").unwrap();
    big.flush().unwrap();
    drop(big);
    let path = std::fs::canonicalize(path).unwrap();
    let (mut harness, requests, _) = wired(&[path], 0, Appearance::default(), Wiring::default());
    for _ in 0..20 {
        settle(&mut harness);
        if harness.count(".viewer-failed") > 0 {
            break;
        }
    }
    let said = harness.text_of(".viewer-failed").unwrap_or_default();
    assert!(
        said.contains("too large"),
        "{said:?} / {:?}",
        harness.text_of(".viewer")
    );
    assert!(!said.contains("Unsupported"), "{said:?}");
    assert!(!said.contains("Open With"), "{said:?}");
    assert!(said.contains("Show in Folder"), "{said:?}");
    let reveal = harness
        .centre(".viewer-failed .ds-button:first-child")
        .expect("Show in Folder");
    harness.send(Input::click(reveal));
    settle(&mut harness);
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| matches!(request, HostRequest::Reveal(_))),
        "Show in Folder asks the host"
    );
}
