//! The viewer offered to docket, the agent layer: the shipped manifest is a valid one, and a
//! `Perform` from the router on a private bus reaches the viewer's own request channel. No real
//! session bus, router or desktop is involved: the test owns `org.quire.Intents1` itself, the name
//! a provider trusts, on a bus of its own.

#![cfg(feature = "quire-desktop")]

use crate::support;

use anyview_core::FilePath;
use anyview_platform::linux::{AGENT_BUS_NAME, DbusInstance, OPEN_FILES};
use anyview_platform::{Claim, Instance, Primary, Request};
use docket_core::{
    Args, CallId, FileRef, Invocation, Manifest, Origin, TargetKind, TargetValue, validate,
};
use porter_core::AppName;
use prov::{ActionName, Actor, Effect, SpaceId};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use support::PrivateBus;
use zbus::zvariant::Value;

const PROVIDER_PATH: &str = "/org/quire/IntentProvider1";
const PROVIDER_INTERFACE: &str = "org.quire.IntentProvider1";
const ROUTER_NAME: &str = "org.quire.Intents1";

fn shipped_manifest() -> String {
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/intents/org.quire.Anyview.toml");
    std::fs::read_to_string(file).unwrap()
}

#[test]
fn the_shipped_manifest_validates_and_only_reads() {
    let manifest: Manifest = toml::from_str(&shipped_manifest()).unwrap();
    let valid = validate(manifest).unwrap();
    let manifest = valid.manifest();
    assert_eq!(manifest.app.as_str(), AGENT_BUS_NAME);
    assert!(!manifest.actions.is_empty());
    // Nothing the viewer offers an agent writes, sends or deletes.
    for action in &manifest.actions {
        assert_eq!(action.effect, Effect::Read, "{}", action.name);
    }
    let open = manifest
        .actions
        .iter()
        .find(|action| action.name.as_str() == OPEN_FILES)
        .unwrap();
    assert_eq!(open.on, TargetKind::Files);
}

fn invocation(files: &[&Path]) -> String {
    let files = files
        .iter()
        .map(|file| FileRef::parse(&file.to_string_lossy()).unwrap())
        .collect();
    let call = Invocation {
        call: CallId(1),
        action: ActionName::parse(OPEN_FILES).unwrap(),
        target: TargetValue::Files(files),
        args: Args::new(),
        actor: Actor::User {
            via: AppName::parse("org.quire.Shell").unwrap(),
        },
        origin: Origin::Launcher,
        space: SpaceId::parse("work").unwrap(),
    };
    serde_json::to_string(&call).unwrap()
}

async fn connect(bus: &PrivateBus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.address())
        .unwrap()
        .build()
        .await
        .unwrap()
}

/// The router's `Perform` on the viewer, as `caller` makes it.
async fn perform(caller: &zbus::Connection, invocation: String) -> zbus::Result<serde_json::Value> {
    let options: HashMap<String, Value<'_>> = HashMap::new();
    let reply = caller
        .call_method(
            Some(AGENT_BUS_NAME),
            PROVIDER_PATH,
            Some(PROVIDER_INTERFACE),
            "Perform",
            &(invocation, options),
        )
        .await?;
    let text: String = reply.body().deserialize()?;
    Ok(serde_json::from_str(&text).unwrap())
}

async fn viewer(bus: &PrivateBus) -> Primary {
    match DbusInstance::new(bus.env())
        .claim(&Request::Open(Vec::new()))
        .await
        .unwrap()
    {
        Claim::Primary(primary) => primary,
        Claim::Forwarded => panic!("the first launch must own the name"),
    }
}

async fn router(bus: &PrivateBus) -> zbus::Connection {
    let router = connect(bus).await;
    router.request_name(ROUTER_NAME).await.unwrap();
    router
}

#[tokio::test]
async fn a_file_open_from_the_router_reaches_the_viewers_open_path() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = viewer(&bus).await;
    let router = router(&bus).await;
    let scratch = tempfile::tempdir().unwrap();
    let (a, b) = (scratch.path().join("a.png"), scratch.path().join("b b.pdf"));
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();

    let answer = perform(&router, invocation(&[&a, &b])).await.unwrap();
    assert!(answer.get("Ok").is_some(), "{answer}");
    let want = Request::Open(vec![
        FilePath::new(a.clone()).unwrap(),
        FilePath::new(b.clone()).unwrap(),
    ]);
    assert_eq!(primary.next_within(Duration::from_secs(5)), Some(want));
}

#[tokio::test]
async fn a_missing_file_refuses_the_whole_call_and_opens_nothing() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = viewer(&bus).await;
    let router = router(&bus).await;
    let scratch = tempfile::tempdir().unwrap();
    let (here, gone) = (
        scratch.path().join("here.png"),
        scratch.path().join("gone.png"),
    );
    std::fs::write(&here, b"a").unwrap();

    let answer = perform(&router, invocation(&[&here, &gone])).await.unwrap();
    assert!(answer.get("Err").is_some(), "{answer}");
    // The next real request is the first thing the viewer hears.
    let ok = perform(&router, invocation(&[&here])).await.unwrap();
    assert!(ok.get("Ok").is_some(), "{ok}");
    assert_eq!(
        primary.next_within(Duration::from_secs(5)),
        Some(Request::Open(vec![FilePath::new(here).unwrap()]))
    );
}

#[tokio::test]
async fn only_the_router_may_call_the_provider() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = viewer(&bus).await;
    let intruder = connect(&bus).await;
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("a.png");
    std::fs::write(&file, b"a").unwrap();

    assert!(perform(&intruder, invocation(&[&file])).await.is_err());
    let heard = primary.next_within(Duration::from_millis(300));
    assert!(
        heard.is_none(),
        "a caller that is not the router opened a file"
    );
}
