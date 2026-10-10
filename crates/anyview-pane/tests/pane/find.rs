//! Find in a pane, through the host's palette: the host feeds the query to the handle, lists the
//! hits it gives back as rows, runs a picked row, and ends the find when it closes its mode.

use crate::support::{Call, Host, hosted};
use anyview_pane::PaneCommand;
use anyview_ui::HitIndex;
use ds_harness::{Query, Viewport};
use std::path::PathBuf;

fn in_a_corner() -> Viewport {
    Viewport {
        width: 480,
        height: 320,
        scale_percent: 100,
    }
}

fn hit(at: u32) -> PaneCommand {
    PaneCommand::FindHit(HitIndex(at))
}

/// Three hundred rows with "needle" in rows 50, 150 and 250.
fn needles(dir: &tempfile::TempDir) -> Host {
    let body: String = (1..=300)
        .map(|n| match n % 100 {
            50 => format!("row {n} has the needle in it\n"),
            _ => format!("row {n} is hay\n"),
        })
        .collect();
    let path = dir.path().join("rows.txt");
    std::fs::write(&path, body).unwrap();
    hosted(
        vec![(std::fs::canonicalize(path).unwrap(), None)],
        in_a_corner(),
    )
}

#[test]
fn a_query_lists_its_hits_a_few_first_and_then_all() {
    let dir = tempfile::tempdir().unwrap();
    let path = crate::support::text_file(dir.path(), "words.txt", "word", 40);
    let mut host = hosted(vec![(path, None)], in_a_corner());
    assert!(host.listed().whole.is_empty(), "no find, no hits");
    // "word 1" is on the lines 1 and 10 to 19.
    host.find("word 1");
    let listed = host.listed();
    let mut brief: Vec<PaneCommand> = (0..8).map(hit).collect();
    brief.push(PaneCommand::ShowAllHits);
    assert_eq!(listed.brief, brief);
    assert_eq!(listed.whole, (0..11).map(hit).collect::<Vec<_>>());
    assert!(
        host.harness.count(".viewer-hit") >= 1,
        "the pane marks them"
    );
}

#[test]
fn running_a_hit_moves_the_pane_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = needles(&dir);
    host.find("needle");
    assert_eq!(host.listed().whole, [hit(0), hit(1), hit(2)]);
    // hit, the row it is on
    for (at, row) in [(2, 250), (1, 150), (0, 50)] {
        host.ask(Call::Run(hit(at)));
        let first = host.first_line().unwrap();
        assert!(
            (row - 20..=row).contains(&first),
            "hit {at} is on row {row}, the pane shows from {first}"
        );
        assert_eq!(host.harness.count(".viewer-hit-current"), 1, "hit {at}");
    }
}

#[test]
fn ending_the_find_clears_its_marks_and_its_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = needles(&dir);
    // how the host ends it
    for end in [Call::EndFind, Call::Find(String::new())] {
        host.find("needle");
        assert!(host.harness.count(".viewer-hit") >= 1, "{end:?}");
        host.ask(end.clone());
        assert_eq!(host.harness.count(".viewer-hit"), 0, "{end:?}");
        assert!(host.listed().whole.is_empty(), "{end:?}");
    }
}
