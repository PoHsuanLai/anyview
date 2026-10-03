//! `Instance` against a private session bus: the first claim owns the name, later ones forward.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FilePath;
use anyview_platform::linux::{BUS_NAME, DbusInstance};
use anyview_platform::{Claim, Instance, Request};
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
