//! `MediaSession` against a private session bus, read the way the control center reads an MPRIS
//! player: through the properties interface and its methods.

#![cfg(feature = "quire-desktop")]
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FilePath, MediaLength, MediaTime, Percent, Volume};
use anyview_platform::linux::{MPRIS_NAME, MprisSession};
use anyview_platform::{
    Ability, MediaControl, MediaSession, MediaState, PlaybackStatus, SeekDirection, TrackSerial,
};
use std::collections::HashMap;
use support::PrivateBus;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

async fn client(bus: &PrivateBus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.address())
        .unwrap()
        .build()
        .await
        .unwrap()
}

async fn get(client: &zbus::Connection, interface: &str, property: &str) -> OwnedValue {
    let reply = client
        .call_method(
            Some(MPRIS_NAME),
            PATH,
            Some(PROPERTIES),
            "Get",
            &(interface, property),
        )
        .await
        .unwrap();
    reply.body().deserialize().unwrap()
}

/// Call a method of the player with no arguments.
async fn press(client: &zbus::Connection, method: &str) {
    client
        .call_method(Some(MPRIS_NAME), PATH, Some(PLAYER), method, &())
        .await
        .unwrap();
}

fn playing() -> MediaState {
    MediaState {
        status: PlaybackStatus::Playing,
        track: TrackSerial(3),
        file: Some(FilePath::new("/music/a b.flac").unwrap()),
        title: Some("Song".to_owned()),
        artist: Some("Band".to_owned()),
        album: None,
        length: Some(MediaLength(MediaTime::from_secs(180))),
        position: MediaTime::from_secs(12),
        volume: Volume::clamped(Percent(80)),
        seek: Ability::Can,
        skip: Ability::Can,
    }
}

#[tokio::test]
async fn the_player_registers_and_shows_what_was_published() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let session = MprisSession::register(&bus.env()).await.unwrap();
    let client = client(&bus).await;

    let identity: String = get(&client, ROOT, "Identity").await.try_into().unwrap();
    assert_eq!(identity, "Viewer");
    let before: String = get(&client, PLAYER, "PlaybackStatus")
        .await
        .try_into()
        .unwrap();
    assert_eq!(before, "Stopped");

    session.publish(&playing()).await.unwrap();

    let status: String = get(&client, PLAYER, "PlaybackStatus")
        .await
        .try_into()
        .unwrap();
    assert_eq!(status, "Playing");
    let position: i64 = get(&client, PLAYER, "Position").await.try_into().unwrap();
    assert_eq!(position, 12_000_000);
    let volume: f64 = get(&client, PLAYER, "Volume").await.try_into().unwrap();
    assert!((volume - 0.8).abs() < 1e-9, "{volume}");
    let metadata: HashMap<String, OwnedValue> =
        get(&client, PLAYER, "Metadata").await.try_into().unwrap();
    let text = |key: &str| -> String { metadata[key].try_clone().unwrap().try_into().unwrap() };
    assert_eq!(text("xesam:title"), "Song");
    assert_eq!(text("xesam:url"), "file:///music/a%20b.flac");
    let length: i64 = metadata["mpris:length"]
        .try_clone()
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(length, 180_000_000);
    let artists: Vec<String> = metadata["xesam:artist"]
        .try_clone()
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(artists, vec!["Band".to_owned()]);
    let track: ObjectPath<'_> = (&*metadata["mpris:trackid"]).try_into().unwrap();
    assert_eq!(track.as_str(), "/org/quire/Anyview1/Track/3");
}

#[tokio::test]
async fn controls_from_the_desktop_arrive_as_typed_inputs() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut session = MprisSession::register(&bus.env()).await.unwrap();
    session.publish(&playing()).await.unwrap();
    let client = client(&bus).await;

    press(&client, "PlayPause").await;
    press(&client, "Next").await;
    press(&client, "Previous").await;
    press(&client, "Stop").await;
    client
        .call_method(
            Some(MPRIS_NAME),
            PATH,
            Some(PLAYER),
            "Seek",
            &(-5_000_000_i64,),
        )
        .await
        .unwrap();
    let track = ObjectPath::try_from("/org/quire/Anyview1/Track/3").unwrap();
    client
        .call_method(
            Some(MPRIS_NAME),
            PATH,
            Some(PLAYER),
            "SetPosition",
            &(track, 30_000_000_i64),
        )
        .await
        .unwrap();
    // A position for a track that is not playing is ignored.
    let other = ObjectPath::try_from("/org/quire/Anyview1/Track/9").unwrap();
    client
        .call_method(
            Some(MPRIS_NAME),
            PATH,
            Some(PLAYER),
            "SetPosition",
            &(other, 1_i64),
        )
        .await
        .unwrap();
    client
        .call_method(
            Some(MPRIS_NAME),
            PATH,
            Some(PROPERTIES),
            "Set",
            &(PLAYER, "Volume", Value::from(0.5_f64)),
        )
        .await
        .unwrap();
    press(&client, "Pause").await;

    let want = [
        MediaControl::PlayPause,
        MediaControl::Next,
        MediaControl::Previous,
        MediaControl::Stop,
        MediaControl::SeekBy(SeekDirection::Backward, MediaTime(5_000_000)),
        MediaControl::SeekTo(MediaTime::from_secs(30)),
        MediaControl::SetVolume(Volume::clamped(Percent(50))),
        MediaControl::Pause,
    ];
    for control in want {
        assert_eq!(session.next_control().await, Some(control));
    }
}

#[tokio::test]
async fn a_control_the_state_does_not_offer_is_not_delivered() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut session = MprisSession::register(&bus.env()).await.unwrap();
    let client = client(&bus).await;
    // Nothing published: skipping and seeking are not offered.
    press(&client, "Next").await;
    client
        .call_method(Some(MPRIS_NAME), PATH, Some(PLAYER), "Seek", &(1_i64,))
        .await
        .unwrap();
    press(&client, "Play").await;
    assert_eq!(session.next_control().await, Some(MediaControl::Play));
    let can_next: bool = get(&client, PLAYER, "CanGoNext").await.try_into().unwrap();
    assert!(!can_next);

    session.publish(&playing()).await.unwrap();
    let can_next: bool = get(&client, PLAYER, "CanGoNext").await.try_into().unwrap();
    assert!(can_next);
}

#[tokio::test]
async fn the_root_interface_asks_the_viewer_to_raise_or_quit() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut session = MprisSession::register(&bus.env()).await.unwrap();
    let client = client(&bus).await;
    for method in ["Raise", "Quit"] {
        client
            .call_method(Some(MPRIS_NAME), PATH, Some(ROOT), method, &())
            .await
            .unwrap();
    }
    assert_eq!(session.next_control().await, Some(MediaControl::Raise));
    assert_eq!(session.next_control().await, Some(MediaControl::Quit));
}
