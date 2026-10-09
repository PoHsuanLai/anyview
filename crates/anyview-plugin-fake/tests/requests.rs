//! Probe, thumbnail and decode round trips, the handshake, and a plugin that misbehaves.

// Test helpers sit outside `#[test]` functions, where the workspace denies `unwrap`; a test may unwrap.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FactLabel, PixelArea, PixelLen};
use anyview_platform::{PlatformError, PluginRunner, Timeouts};
use anyview_plugin_protocol::{Capability, ErrorCode};
use std::time::{Duration, Instant};
use support::{Scratch, Where};

fn quick() -> PluginRunner {
    PluginRunner::new(
        Timeouts::default()
            .with_hello(Duration::from_millis(400))
            .with_silence(Duration::from_millis(400))
            .with_cancel_grace(Duration::from_millis(400)),
    )
}

fn rows(rows: &[anyview_plugin_protocol::FactRow]) -> Vec<(String, String)> {
    rows.iter()
        .map(|row| (row.label.clone(), row.value.clone()))
        .collect()
}

#[test]
fn probe_returns_the_plugins_rows() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let path = scratch.book("a.book", "Moby Dick");
    let got = PluginRunner::default()
        .probe(&scratch.plugin("fake"), &path)
        .unwrap();
    assert_eq!(
        rows(&got)[..2],
        [
            ("kind".to_owned(), "Fake book".to_owned()),
            ("title".to_owned(), "Moby Dick".to_owned()),
        ]
    );
}

#[test]
fn a_plugin_error_reaches_the_caller_with_its_code() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let missing = anyview_core::FilePath::new(scratch.dir.path().join("absent")).unwrap();
    let error = PluginRunner::default()
        .probe(&scratch.plugin("fake"), &missing)
        .unwrap_err();
    assert!(
        matches!(
            &error,
            PlatformError::PluginFailed {
                code: ErrorCode::Unreadable,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn thumbnail_returns_pixels_inside_the_edge() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let path = scratch.book("a.book", "x");
    let runner = PluginRunner::default();
    let small = runner
        .thumbnail(&scratch.plugin("fake"), &path, PixelLen(8))
        .unwrap();
    assert_eq!((small.size().width.0, small.size().height.0), (8, 8));
    // Pixel (3, 2) of the gradient is [3, 2, 0, 255].
    assert_eq!(&small.rgba()[(2 * 8 + 3) * 4..][..4], [3, 2, 0, 255]);
    let big = runner
        .thumbnail(&scratch.plugin("fake"), &path, PixelLen(256))
        .unwrap();
    assert_eq!(big.size().width.0, 16);
}

#[test]
fn a_24_megapixel_decode_crosses_the_pipe_whole_and_fast_enough() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let path = scratch.book("a.book", "x");
    let runner = PluginRunner::new(
        Timeouts::default()
            .with_hello(Duration::from_secs(5))
            .with_silence(Duration::from_secs(60))
            .with_cancel_grace(Duration::from_secs(1)),
    );
    let started = Instant::now();
    let picture = runner
        .decode(&scratch.plugin("fake"), &path, PixelArea(30_000_000))
        .unwrap();
    let took = started.elapsed();
    assert_eq!(picture.size().area(), PixelArea(24_000_000));
    assert_eq!(picture.rgba().len(), 96_000_000);
    // The last pixel of the gradient: (5999 % 256, 3999 % 256).
    assert_eq!(
        &picture.rgba()[picture.rgba().len() - 4..],
        [5999 % 256, 3999 % 256, 0, 255].map(|v| v as u8)
    );
    assert!(took < Duration::from_secs(10), "took {took:?}");
}

#[test]
fn a_decode_is_scaled_to_the_budget_the_host_gave() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let path = scratch.book("a.book", "x");
    let picture = PluginRunner::default()
        .decode(&scratch.plugin("fake"), &path, PixelArea(1_000_000))
        .unwrap();
    assert!(picture.size().area() <= PixelArea(1_000_000));
    assert!(picture.size().area() > PixelArea(900_000));
}

#[test]
fn a_protocol_version_the_viewer_does_not_speak_is_refused() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "other-version"]);
    let path = scratch.book("a.book", "x");
    let error = quick().probe(&scratch.plugin("fake"), &path).unwrap_err();
    assert_eq!(
        error,
        PlatformError::PluginVersion {
            plugin: "fake".to_owned(),
            offered: 99,
            supported: 1
        }
    );
}

#[test]
fn a_plugin_that_does_not_list_the_capability_is_refused() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "provides-nothing"]);
    let path = scratch.book("a.book", "x");
    let error = quick().probe(&scratch.plugin("fake"), &path).unwrap_err();
    assert_eq!(
        error,
        PlatformError::PluginLacks {
            plugin: "fake".to_owned(),
            capability: Capability::Probe
        }
    );
}

#[test]
fn a_misbehaving_plugin_is_a_typed_error_and_never_hangs_the_host() {
    type Check = fn(&PlatformError) -> bool;
    const CASES: &[(&str, &str, Check)] = &[
        ("silent at hello", "mute", |e| {
            matches!(e, PlatformError::PluginSilent { .. })
        }),
        ("hangs after the request", "hang-on-request", |e| {
            matches!(e, PlatformError::PluginSilent { .. })
        }),
        ("crashes with a status", "crash-on-request", |e| {
            matches!(e, PlatformError::PluginCrashed { status, .. }
                if status.contains("3") && status.contains("crashing on purpose"))
        }),
        ("writes garbage", "garbage", |e| {
            matches!(e, PlatformError::PluginProtocol { .. })
        }),
    ];
    for (name, fault, check) in CASES {
        let scratch = Scratch::new();
        scratch.install(Where::User, "fake", 1, &["--fault", fault]);
        let path = scratch.book("a.book", "x");
        let started = Instant::now();
        let error = quick().probe(&scratch.plugin("fake"), &path).unwrap_err();
        assert!(check(&error), "{name}: {error:?}");
        assert!(started.elapsed() < Duration::from_secs(3), "{name}");
    }
}

#[test]
fn a_picture_of_the_wrong_size_is_a_protocol_error() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "short-picture"]);
    let path = scratch.book("a.book", "x");
    let error = quick()
        .thumbnail(&scratch.plugin("fake"), &path, PixelLen(8))
        .unwrap_err();
    assert!(
        matches!(error, PlatformError::PluginProtocol { .. }),
        "{error:?}"
    );
}

#[test]
fn a_header_announcing_more_pixels_than_asked_for_is_refused_before_they_arrive() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "huge-payload"]);
    let path = scratch.book("a.book", "x");
    let started = Instant::now();
    // Timeouts of five seconds: a wait for the payload would end after all of them, so ending
    // well before is the refusal, whatever the load of the machine.
    let patient = PluginRunner::new(
        Timeouts::default()
            .with_hello(Duration::from_secs(5))
            .with_silence(Duration::from_secs(5))
            .with_cancel_grace(Duration::from_secs(5)),
    );
    let error = patient
        .thumbnail(&scratch.plugin("fake"), &path, PixelLen(8))
        .unwrap_err();
    // A protocol error, not the silence a wait for the payload would end in.
    assert!(
        matches!(error, PlatformError::PluginProtocol { .. }),
        "{error:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn a_plugin_that_floods_stderr_without_a_newline_still_answers() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "stderr-flood"]);
    let path = scratch.book("a.book", "x");
    let got = PluginRunner::default()
        .thumbnail(&scratch.plugin("fake"), &path, PixelLen(8))
        .unwrap();
    assert_eq!(got.size().width.0, 8);
}

#[test]
fn a_program_that_cannot_be_started_is_a_spawn_error() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let plugin = scratch.plugin("fake");
    // The manifest was valid when discovered; the program vanishes before it is run.
    let mut gone = plugin.clone();
    gone.manifest.program.as_mut().unwrap().path = "/nonexistent/program".into();
    let path = scratch.book("a.book", "x");
    let error = PluginRunner::default().probe(&gone, &path).unwrap_err();
    assert!(matches!(error, PlatformError::Spawn { .. }), "{error:?}");
}

#[test]
fn probe_facts_keep_the_labels_the_viewer_knows() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let path = scratch.book("a.book", "Dune");
    let plugins = scratch.plugins();
    let answer = PluginRunner::default()
        .peek_facts(&plugins, &support::book(), &path)
        .unwrap();
    let anyview_platform::PluginFacts::Facts(facts) = answer else {
        panic!("facts expected, got {answer:?}");
    };
    let labels: Vec<_> = facts.rows().iter().map(|fact| fact.label).collect();
    assert_eq!(labels, [FactLabel::Kind, FactLabel::Title]);
    assert_eq!(facts.value(FactLabel::Title).unwrap().as_str(), "Dune");
}
