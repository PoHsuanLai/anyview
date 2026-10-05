//! The table and tree stages under the harness: a very large table mounts only the rows near the
//! window, a tree shows its first level, and the headers come from the file.

#![allow(clippy::unwrap_used)]

mod support;

use ds::prelude::{Appearance, Point, Px};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use std::time::Duration;
use support::{VIEW, window};

fn settle(harness: &mut Harness) {
    harness.advance(Duration::from_millis(300));
}

fn open(name: &str, body: &str) -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join(name);
    std::fs::write(&path, body).unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let (mut harness, _) = window(&[path], 0, Appearance::default());
    settle(&mut harness);
    harness.send(Input::pointer_move(Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }));
    settle(&mut harness);
    (dir, harness)
}

#[test]
fn a_table_of_a_hundred_thousand_rows_mounts_only_a_window_of_them() {
    let mut body = String::from("id,name,score\n");
    for n in 0..100_000 {
        body.push_str(&format!("{n},name {n},{}\n", n % 97));
    }
    let (_dir, harness) = open("big.csv", &body);
    let mounted = harness.count(".viewer-row-number");
    assert!(mounted > 0, "the first rows are drawn");
    assert!(mounted < 200, "{mounted} rows are mounted for 100000");
    assert_eq!(harness.text_of(".ds-table-title").as_deref(), Some("#"));
}

#[test]
fn a_json_file_shows_as_a_tree_of_its_top_level() {
    let (_dir, harness) = open("tree.json", r#"{"name":"anyview","tags":["a","b"],"n":3}"#);
    assert!(harness.count(".viewer-node") >= 3, "the keys are rows");
    assert!(harness.text_of(".viewer-node-key").is_some());
}
