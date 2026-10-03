use super::*;
use crate::window::Opening;
use anyview_core::{FilePath, NonEmpty, ResultsId, Resume, Sequence, SequenceOrigin};
use anyview_platform::linux::DbusInstance;
use anyview_platform::testing::{FakeInstance, FakeRole};
use anyview_platform::{Env, Handoff, Request};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use std::time::Duration;

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

#[test]
fn a_request_is_the_windows_to_open_each_with_its_own_file() {
    let results = Sequence::starting_at(
        NonEmpty::from_vec(vec![path("/a/1.pdf"), path("/a/2.pdf")]).unwrap(),
        &path("/a/2.pdf"),
        SequenceOrigin::Results(ResultsId(5)),
    )
    .unwrap();
    let handed = Handoff {
        file: path("/a/2.pdf"),
        resume: Resume::Nothing,
        sequence: Some(results),
    };
    let around = |text: &str| Wanted::Around(path(text));
    let cases = [
        (
            "files keep their order",
            Request::Open(vec![path("/a/1.png"), path("/a/2.png")]),
            vec![around("/a/1.png"), around("/a/2.png")],
        ),
        (
            "showing the window names no file",
            Request::Open(vec![]),
            vec![],
        ),
        (
            "a peek opens like an open",
            Request::Peek(path("/a/1.pdf")),
            vec![around("/a/1.pdf")],
        ),
        (
            "so does a play",
            Request::Play(path("/a/1.mp3")),
            vec![around("/a/1.mp3")],
        ),
        (
            "a handoff keeps its results and its place",
            Request::Handoff(handed.clone()),
            vec![Wanted::Handed(handed)],
        ),
    ];
    for (name, request, want) in cases {
        assert_eq!(wanted_by(request), want, "{name}");
    }
}

#[tokio::test]
async fn the_first_launch_is_the_viewer_and_the_second_forwards_its_request_and_leaves() {
    let request = Request::Open(vec![path("/a/1.png")]);

    let first = FakeInstance::new(FakeRole::FirstLaunch);
    let role = claim_role(&first, &request).await;
    assert!(matches!(role, Role::Primary(_)), "{role:?}");
    assert_eq!(first.claimed(), vec![request.clone()]);

    let second = FakeInstance::new(FakeRole::SecondLaunch);
    let role = claim_role(&second, &request).await;
    assert!(matches!(role, Role::Forwarded), "{role:?}");
    assert_eq!(
        second.claimed(),
        vec![request],
        "the request went to the running viewer"
    );
}

#[tokio::test]
async fn with_no_bus_the_launch_runs_alone_rather_than_dropping_its_request() {
    let scratch = tempfile::tempdir().unwrap();
    let instance = DbusInstance::new(Env::isolated(scratch.path()));
    let role = claim_role(&instance, &Request::Open(vec![path("/a/1.png")])).await;
    assert!(
        matches!(role, Role::Alone(anyview_platform::PlatformError::NoBus)),
        "{role:?}"
    );
}

#[tokio::test]
async fn a_request_forwarded_to_the_viewer_arrives_as_an_opening_per_file() {
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<FilePath> = ["b.txt", "a.txt"]
        .iter()
        .map(|name| {
            std::fs::write(dir.path().join(name), "x").unwrap();
            path(dir.path().join(name).to_str().unwrap())
        })
        .collect();
    let instance = FakeInstance::new(FakeRole::FirstLaunch);
    let Role::Primary(primary) = claim_role(&instance, &Request::Open(vec![])).await else {
        panic!("the first launch is the viewer");
    };
    let (openings, mut arrived) = unbounded::<Opening>();
    let serving = tokio::spawn(relay(primary, openings));

    instance.forward(Request::Open(files.clone()));
    let mut got = Vec::new();
    for _ in 0..files.len() {
        let opening = tokio::time::timeout(Duration::from_secs(10), arrived.next())
            .await
            .unwrap()
            .unwrap();
        got.push(opening);
    }
    serving.abort();

    assert_eq!(
        got.iter()
            .map(|opening| opening.file.clone())
            .collect::<Vec<_>>(),
        files,
        "one opening per file, in the order asked"
    );
    for opening in &got {
        let sequence = opening.sequence.as_ref().unwrap();
        assert_eq!(
            sequence.entries().count().get(),
            2,
            "the folder's two files"
        );
        assert_eq!(sequence.current(), &opening.file);
    }
}

#[tokio::test]
async fn a_handoff_forwarded_to_the_viewer_opens_the_results_it_names_not_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<FilePath> = ["a.pdf", "b.pdf", "c.pdf"]
        .iter()
        .map(|name| {
            std::fs::write(dir.path().join(name), "x").unwrap();
            path(dir.path().join(name).to_str().unwrap())
        })
        .collect();
    let results = Sequence::starting_at(
        NonEmpty::from_vec(vec![files[2].clone(), files[0].clone()]).unwrap(),
        &files[0],
        SequenceOrigin::Results(ResultsId(9)),
    )
    .unwrap();
    let instance = FakeInstance::new(FakeRole::FirstLaunch);
    let Role::Primary(primary) = claim_role(&instance, &Request::Open(vec![])).await else {
        panic!("the first launch is the viewer");
    };
    let (openings, mut arrived) = unbounded::<Opening>();
    let serving = tokio::spawn(relay(primary, openings));

    instance.forward(Request::Handoff(Handoff {
        file: files[0].clone(),
        resume: Resume::Nothing,
        sequence: Some(results.clone()),
    }));
    let opening = tokio::time::timeout(Duration::from_secs(10), arrived.next())
        .await
        .unwrap()
        .unwrap();
    serving.abort();

    assert_eq!(opening.file, files[0]);
    assert_eq!(
        opening.sequence,
        Some(results),
        "the walk is the search's results, not the three files of the folder"
    );
}
