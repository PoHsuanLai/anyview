//! Export: progress, cancel, timeout and crash.

// Test helpers sit outside `#[test]` functions, where the workspace denies `unwrap`; a test may unwrap.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::work::Stop;
use anyview_platform::{PlatformError, PluginRunner, Timeouts};
use anyview_plugin_protocol::{ErrorCode, ExportRequest, Progress};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use support::{Scratch, Where};

fn request(scratch: &Scratch, target: &str) -> (ExportRequest, PathBuf) {
    let input = scratch.book("a.book", "Dune");
    let output = scratch.dir.path().join("out.txt");
    let request = ExportRequest {
        input: input.as_path().to_path_buf(),
        output: output.clone(),
        target: target.to_owned(),
        range: None,
        stream: None,
        bitrate: None,
    };
    (request, output)
}

fn runner() -> PluginRunner {
    PluginRunner::new(Timeouts {
        hello: Duration::from_secs(2),
        silence: Duration::from_secs(2),
        cancel_grace: Duration::from_millis(400),
    })
}

#[test]
fn an_export_reports_progress_then_finishes_with_the_file_written() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let (request, output) = request(&scratch, "txt");
    let mut seen = Vec::new();
    let done = runner()
        .export(&scratch.plugin("fake"), &request, &Stop::new(), |p| {
            seen.push(p)
        })
        .unwrap();
    assert_eq!(done.output, None);
    assert_eq!(seen.len(), 10);
    assert_eq!(seen[0], Progress { done: 1, total: 10 });
    assert_eq!(
        seen[9],
        Progress {
            done: 10,
            total: 10
        }
    );
    assert_eq!(std::fs::read_to_string(output).unwrap(), "exported: Dune\n");
}

#[test]
fn a_target_the_plugin_does_not_write_is_its_own_error() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let (request, _) = request(&scratch, "flac");
    let error = runner()
        .export(&scratch.plugin("fake"), &request, &Stop::new(), |_| {})
        .unwrap_err();
    assert!(
        matches!(
            error,
            PlatformError::PluginFailed {
                code: ErrorCode::Unsupported,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn cancel_stops_the_plugin_and_it_removes_its_partial_output() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &[]);
    let (request, output) = request(&scratch, "txt");
    let stop = Stop::new();
    let raise = stop.clone();
    let mut steps = 0;
    let error = runner()
        .export(&scratch.plugin("fake"), &request, &stop, |_| {
            steps += 1;
            if steps == 2 {
                raise.request();
            }
        })
        .unwrap_err();
    assert_eq!(
        error,
        PlatformError::PluginCancelled {
            plugin: "fake".to_owned()
        }
    );
    assert!(steps < 10, "it stopped early, after {steps} steps");
    assert!(!output.exists());
    assert!(!output.with_extension("partial").exists());
}

#[test]
fn a_plugin_that_ignores_cancel_is_killed_after_the_grace_period() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "ignore-cancel"]);
    let (request, _) = request(&scratch, "txt");
    let stop = Stop::new();
    stop.request();
    let started = Instant::now();
    let error = runner()
        .export(&scratch.plugin("fake"), &request, &stop, |_| {})
        .unwrap_err();
    assert_eq!(
        error,
        PlatformError::PluginCancelled {
            plugin: "fake".to_owned()
        }
    );
    // The export would take 400 ms; the grace period is 400 ms from the cancel.
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_plugin_that_goes_quiet_during_an_export_times_out() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "hang-on-request"]);
    let (request, _) = request(&scratch, "txt");
    let runner = PluginRunner::new(Timeouts {
        hello: Duration::from_secs(2),
        silence: Duration::from_millis(300),
        cancel_grace: Duration::from_millis(300),
    });
    let error = runner
        .export(&scratch.plugin("fake"), &request, &Stop::new(), |_| {})
        .unwrap_err();
    assert!(
        matches!(error, PlatformError::PluginSilent { .. }),
        "{error:?}"
    );
}

#[test]
fn a_crash_mid_export_is_a_typed_error_with_what_it_said() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "fake", 1, &["--fault", "crash-in-export"]);
    let (request, output) = request(&scratch, "txt");
    let mut seen = 0;
    let error = runner()
        .export(&scratch.plugin("fake"), &request, &Stop::new(), |_| {
            seen += 1
        })
        .unwrap_err();
    assert_eq!(seen, 1, "one progress message before the crash");
    let PlatformError::PluginCrashed { plugin, status } = error else {
        panic!("a crash, got {error:?}");
    };
    assert_eq!(plugin, "fake");
    assert!(status.contains("4"), "{status}");
    assert!(status.contains("crashing mid export"), "{status}");
    assert!(!output.exists(), "the output is written only on success");
}
