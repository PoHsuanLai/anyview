use super::*;
use anyview_core::FilePath;
use anyview_platform::linux::DbusInstance;
use anyview_platform::testing::{FakeInstance, FakeRole};
use anyview_platform::{Env, Request};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use std::time::Duration;

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

#[test]
fn a_request_is_what_is_wanted_of_each_file() {
    let cases = [
        (
            "files keep their order, each for a window",
            Request::Open(vec![path("/a/1.png"), path("/a/2.png")]),
            vec![Want::Show(path("/a/1.png")), Want::Show(path("/a/2.png"))],
        ),
        (
            "showing the window names no file",
            Request::Open(vec![]),
            vec![],
        ),
        (
            "a peek opens like an open",
            Request::Peek(path("/a/1.pdf")),
            vec![Want::Show(path("/a/1.pdf"))],
        ),
        (
            "a play has no window",
            Request::Play(path("/a/1.mp3")),
            vec![Want::Play(path("/a/1.mp3"))],
        ),
    ];
    for (name, request, want) in cases {
        assert_eq!(wants_of(request), want, "{name}");
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
    let (openings, mut arrived) = unbounded::<Arrival>();
    let serving = tokio::spawn(relay(primary, openings));

    instance.forward(Request::Open(files.clone()));
    let mut got = Vec::new();
    for _ in 0..files.len() {
        let Arrival::Window(opening) =
            tokio::time::timeout(Duration::from_secs(10), arrived.next())
                .await
                .unwrap()
                .unwrap()
        else {
            panic!("an open is a window");
        };
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
