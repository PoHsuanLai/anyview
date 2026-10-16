//! Tests of the work a pool runs and the edge that submits it.

use super::super::job::{Done, Job, Replaces};
use super::super::seam_set::Seams;
use super::edge::Edge;
use super::work::{Work, Workers};
use crate::{LoadFailure, Ticket};
use anyview_core::work::StopState;
use anyview_core::{FilePath, Resume};
use std::sync::Arc;

use crate::io::{OpenError, ResumeSource};
use anyview_core::FileStamp;

/// A host store that falls over when asked, as a decoder does on a hostile file.
#[derive(Debug)]
struct Falling;

impl ResumeSource for Falling {
    fn recall(&self, _path: &FilePath, _stamp: FileStamp) -> Resume {
        panic!("the store fell over");
    }
}

/// A pool of no threads: the work runs where it is submitted.
struct Inline;

impl Workers for Inline {
    fn submit(&self, work: Work) {
        work.run();
    }
}

#[test]
fn a_job_that_panics_answers_its_load_with_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, b"hello\n").unwrap();
    let path = FilePath::new(&file).unwrap();
    let seams = Seams::default().with_resume_source(Arc::new(Falling));
    let edge = Edge::new(Arc::new(Inline), |_| {}, seams);
    edge.submit(Job::Probe {
        ticket: Ticket(1),
        path,
    });
    let mut mailbox = edge.take_mailbox().unwrap();
    let Ok(Done::Probed { ticket, result }) = mailbox.try_recv() else {
        panic!("the abandoned probe left no answer");
    };
    assert_eq!(ticket, Ticket(1));
    let error = result.unwrap_err();
    assert!(matches!(error, OpenError::Crashed));
    assert_eq!(error.failure(), LoadFailure::Damaged);
}

fn edge() -> Edge {
    struct Nowhere;
    impl Workers for Nowhere {
        fn submit(&self, _work: Work) {}
    }
    Edge::new(Arc::new(Nowhere), |_| {}, Seams::default())
}

#[test]
fn a_newer_load_stops_the_work_of_the_one_it_replaced() {
    let edge = edge();
    let first = edge.stop_for(Ticket(1));
    let same = edge.stop_for(Ticket(1));
    assert_eq!(first.stopped(), StopState::Running);
    assert_eq!(same.stopped(), StopState::Running, "one load shares a stop");
    let second = edge.stop_for(Ticket(2));
    assert_eq!(first.stopped(), StopState::Stopped);
    assert_eq!(same.stopped(), StopState::Stopped);
    assert_eq!(second.stopped(), StopState::Running);
}

#[test]
fn a_newer_search_stops_the_older_one_and_a_newer_load_stops_the_search() {
    let edge = edge();
    let load = edge.stop_for_job(Ticket(1), Replaces::Nothing);
    let older = edge.stop_for_job(Ticket(1), Replaces::Search);
    assert_eq!(older.stopped(), StopState::Running);
    let newer = edge.stop_for_job(Ticket(1), Replaces::Search);
    assert_eq!(
        older.stopped(),
        StopState::Stopped,
        "a newer query replaced it"
    );
    assert_eq!(newer.stopped(), StopState::Running);
    assert_eq!(load.stopped(), StopState::Running, "the load goes on");
    let scrolled_past = edge.stop_for_job(Ticket(1), Replaces::Lines);
    let visible = edge.stop_for_job(Ticket(1), Replaces::Lines);
    assert_eq!(scrolled_past.stopped(), StopState::Stopped);
    assert_eq!(visible.stopped(), StopState::Running);
    assert_eq!(
        newer.stopped(),
        StopState::Running,
        "a window is not a query"
    );
    edge.stop_for_job(Ticket(2), Replaces::Nothing);
    assert_eq!(
        newer.stopped(),
        StopState::Stopped,
        "a newer load ends its searches"
    );
    assert_eq!(
        edge.stop_for_job(Ticket(1), Replaces::Search).stopped(),
        StopState::Stopped,
        "a search of a load already left starts stopped"
    );
}

#[test]
fn work_of_a_load_already_left_starts_stopped_and_loadless_work_never_is() {
    let edge = edge();
    let current = edge.stop_for(Ticket(5));
    assert_eq!(edge.stop_for(Ticket(4)).stopped(), StopState::Stopped);
    let ahead = edge.stop_for(Ticket::default());
    assert_eq!(ahead.stopped(), StopState::Running);
    assert_eq!(
        current.stopped(),
        StopState::Running,
        "loadless work stops no load"
    );
}

#[test]
fn a_preload_ends_when_its_file_is_no_longer_next_to_the_open_one() {
    let edge = edge();
    let path = |name: &str| FilePath::new(format!("/p/{name}")).unwrap();
    let kept = edge.stop_for_preload(&path("a"));
    let passed = edge.stop_for_preload(&path("b"));
    edge.preloads_wanted(&[path("a")]);
    assert_eq!(kept.stopped(), StopState::Running);
    assert_eq!(passed.stopped(), StopState::Stopped);
    let again = edge.stop_for_preload(&path("a"));
    assert_eq!(kept.stopped(), StopState::Stopped, "asked again, replaced");
    assert_eq!(again.stopped(), StopState::Running);
}
