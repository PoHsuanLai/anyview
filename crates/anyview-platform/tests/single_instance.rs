//! `Instance` against a private session bus: the first claim owns the name, later ones forward.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FilePath, LineIndex, NonEmpty, ResultsId, Resume, Sequence, SequenceOrigin};
use anyview_platform::linux::{BUS_NAME, DbusInstance, forward_over};
use anyview_platform::{Claim, Handoff, Instance, Request};
use support::PrivateBus;

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

#[tokio::test]
async fn a_second_launch_forwards_open_peek_and_play_to_the_first() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let first = DbusInstance::new(bus.env());
    let mut primary = match first.claim(&Request::Open(Vec::new())).await.unwrap() {
        Claim::Primary(primary) => primary,
        Claim::Forwarded => panic!("the first launch must own the name"),
    };

    let second = DbusInstance::new(bus.env());
    let sent = [
        Request::Open(vec![path("/pics/a.png"), path("/pics/b b.png")]),
        Request::Peek(path("/docs/c.pdf")),
        Request::Play(path("/music/d.flac")),
        Request::Open(Vec::new()),
    ];
    for request in &sent {
        let claim = second.claim(request).await.unwrap();
        assert!(matches!(claim, Claim::Forwarded), "{request:?}");
    }
    for request in sent {
        assert_eq!(primary.next().await, Some(request));
    }
}

#[tokio::test]
async fn a_request_naming_a_relative_path_is_refused_by_the_running_viewer() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let _primary = DbusInstance::new(bus.env())
        .claim(&Request::Open(Vec::new()))
        .await
        .unwrap();
    let connection = zbus::connection::Builder::address(bus.address())
        .unwrap()
        .build()
        .await
        .unwrap();
    let reply = connection
        .call_method(
            Some(BUS_NAME),
            "/org/quire/Anyview1",
            Some(BUS_NAME),
            "Peek",
            &("relative/file.png",),
        )
        .await;
    assert!(reply.is_err(), "a relative path must not become a request");
}

#[tokio::test]
async fn without_a_bus_the_claim_says_so() {
    let scratch = tempfile::tempdir().unwrap();
    let env = anyview_platform::Env::isolated(scratch.path());
    let error = DbusInstance::new(env)
        .claim(&Request::Open(Vec::new()))
        .await
        .unwrap_err();
    assert_eq!(error, anyview_platform::PlatformError::NoBus);
}

fn handed(place: Resume) -> Handoff {
    let (a, b) = (path("/docs/a.md"), path("/docs/b c.md"));
    let results = Sequence::starting_at(
        NonEmpty::from_vec(vec![a, b.clone()]).unwrap(),
        &b,
        SequenceOrigin::Results(ResultsId(12)),
    )
    .unwrap();
    Handoff {
        file: b,
        resume: place,
        sequence: Some(results),
    }
}

#[tokio::test]
async fn a_handoff_arrives_with_its_place_and_the_results_it_came_from() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = match DbusInstance::new(bus.env())
        .claim(&Request::Open(Vec::new()))
        .await
        .unwrap()
    {
        Claim::Primary(primary) => primary,
        Claim::Forwarded => panic!("the first launch must own the name"),
    };

    let sent = [
        handed(Resume::Text {
            line: LineIndex(40),
        }),
        handed(Resume::Nothing),
        Handoff {
            file: path("/docs/a.md"),
            resume: Resume::Nothing,
            sequence: None,
        },
    ];
    // The launcher is not a viewer: it calls over a connection of its own.
    let launcher = zbus::connection::Builder::address(bus.address())
        .unwrap()
        .build()
        .await
        .unwrap();
    for handoff in &sent {
        forward_over(&launcher, &Request::Handoff(handoff.clone()))
            .await
            .unwrap();
    }
    for handoff in sent {
        assert_eq!(primary.next().await, Some(Request::Handoff(handoff)));
    }
}

#[tokio::test]
async fn a_handoff_whose_file_is_not_among_its_results_is_refused_by_the_viewer() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let _primary = DbusInstance::new(bus.env())
        .claim(&Request::Open(Vec::new()))
        .await
        .unwrap();
    let connection = zbus::connection::Builder::address(bus.address())
        .unwrap()
        .build()
        .await
        .unwrap();
    let call = async |file: &str, resume: &str, entries: Vec<String>| {
        connection
            .call_method(
                Some(BUS_NAME),
                "/org/quire/Anyview1",
                Some(BUS_NAME),
                "Handoff",
                &(file.to_owned(), resume.to_owned(), 1_u64, entries),
            )
            .await
    };
    let nothing = r#"{"kind":"nothing"}"#;
    assert!(
        call("/a.md", nothing, vec!["/b.md".to_owned()])
            .await
            .is_err(),
        "a file outside its results"
    );
    assert!(
        call("/a.md", "line 3", Vec::new()).await.is_err(),
        "a place that is not a resume"
    );
    assert!(
        call("a.md", nothing, Vec::new()).await.is_err(),
        "a relative file"
    );
    assert!(
        call("/a.md", nothing, vec!["/a.md".to_owned()])
            .await
            .is_ok(),
        "the same call with its file among the results is accepted"
    );
}
