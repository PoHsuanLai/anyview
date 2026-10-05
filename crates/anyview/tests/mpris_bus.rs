//! The real `MprisSession` on a private session bus, with the hub and a real player behind it,
//! read the way the control center reads an MPRIS player: a session with no window plays and the
//! entry says so on the bus, a Pause call over the bus pauses it, and Stop ends it. Sound is off.
//! A machine without `dbus-daemon` or a graphics adapter skips it, saying so.

#![allow(clippy::unwrap_used)]

mod support;

use anyview::media::{MediaHub, NowPlaying};
use anyview_core::Resume;
use anyview_media::AudioDriver;
use anyview_platform::testing::{MprisClient, PrivateBus};
use std::time::Duration;
use support::{eventually, media_fixture, probed};

#[test]
fn the_control_center_sees_a_background_session_pauses_it_and_stops_it() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let Some(plugins) = support::plugins(true, false) else {
        return;
    };
    let env = bus.env();
    let hub = MediaHub::start(
        runtime.handle(),
        move || async move { NowPlaying::register(&env).await },
        None,
        AudioDriver::Null,
        plugins,
    );
    let client = runtime.block_on(MprisClient::connect(&bus)).unwrap();
    let status = |client: &MprisClient| runtime.block_on(client.status()).unwrap_or_default();

    let source = probed(&media_fixture("tone.flac"));
    if let Err(error) = hub.play_in_background(&source.source, &source.sniffed, &Resume::Nothing) {
        eprintln!("SKIPPED: cannot start a background session ({error})");
        return;
    }
    // The player is on the bus once something plays, and not before.
    let wait = |what: &str, want: &str| {
        eventually(what, || {
            std::thread::sleep(Duration::from_millis(30));
            status(&client) == want
        });
    };
    wait("the player to say it plays", "Playing");
    runtime.block_on(client.press("Pause")).unwrap();
    wait("the pause that came over the bus", "Paused");
    runtime.block_on(client.press("PlayPause")).unwrap();
    wait("the play that came over the bus", "Playing");
    runtime.block_on(client.press("Stop")).unwrap();
    eventually("the session to end", || !hub.plays_in_background());
    wait("the player to say it stopped", "Stopped");
}
