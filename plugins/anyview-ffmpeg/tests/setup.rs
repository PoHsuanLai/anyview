//! What the plugin says when it starts: its greeting with the real FFmpeg, with one that lacks an
//! encoder, and with none; and that the shipped manifest lists every target it can write.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_platform::{PlatformError, PluginRunner};
use anyview_plugin::{Provision, TargetName};
use anyview_plugin_protocol::{Capability, Hello, PluginMessage, read_frame};
use std::io::Read;
use std::process::{Command, Stdio};
use support::{Scratch, file, fixture};

/// Starts the plugin with `args`, reads its greeting and what it logged, and ends it.
fn greeting(args: &[String]) -> (Hello, String) {
    let mut child = Command::new(support::PLUGIN)
        .args(args)
        .env_remove("ANYVIEW_FFMPEG")
        .env_remove("ANYVIEW_FFPROBE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let frame = read_frame::<_, PluginMessage>(&mut stdout).unwrap();
    // A plugin with nothing to offer exits on its own; one that serves waits for a request.
    drop(child.stdin.take());
    let _ = child.kill();
    let _ = child.wait();
    let mut log = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut log)
        .unwrap();
    let PluginMessage::Hello(hello) = frame.message else {
        panic!("the first message is a hello");
    };
    (hello, log)
}

#[test]
fn with_ffmpeg_it_offers_everything_and_every_target_it_lists_is_in_the_manifest() {
    require_ffmpeg!();
    let (hello, log) = greeting(&[]);
    assert_eq!(hello.name, "ffmpeg");
    assert_eq!(
        hello.provides,
        [
            Capability::Probe,
            Capability::Thumbnail,
            Capability::Decode,
            Capability::Export
        ]
    );
    // Fedora's ffmpeg-free has aac, libmp3lame, flac, pcm and libopus.
    for target in ["trim", "audio-copy", "m4a", "mp3", "flac", "wav", "opus"] {
        assert!(
            hello.targets.iter().any(|t| t == target),
            "{target} in {:?}",
            hello.targets
        );
    }
    let scratch = Scratch::new();
    let plugin = scratch.install(&[]);
    let Some(Provision::Export(manifest)) = plugin.manifest.provision(Capability::Export) else {
        panic!("the manifest provides export");
    };
    let listed: Vec<&str> = manifest.targets.iter().map(TargetName::as_str).collect();
    for target in &hello.targets {
        assert!(
            listed.contains(&target.as_str()),
            "{target} is in the manifest"
        );
    }
    assert!(log.is_empty(), "nothing is logged when all is well: {log}");
}

#[test]
fn an_ffmpeg_without_an_encoder_lists_only_the_targets_it_can_write() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let fake = scratch.fake_ffmpeg("exit 0");
    let (hello, _) = greeting(&support::with_ffmpeg(&fake));
    assert_eq!(hello.targets, ["trim", "audio-copy", "flac"]);
    assert!(hello.provides.contains(&Capability::Export));
}

#[test]
fn without_ffmpeg_it_lists_nothing_logs_why_and_the_host_is_told_it_lacks_the_capability() {
    let scratch = Scratch::new();
    let args = vec!["--ffmpeg".to_owned(), "/nonexistent/ffmpeg".to_owned()];
    let (hello, log) = greeting(&args);
    assert!(
        hello.provides.is_empty() && hello.targets.is_empty(),
        "{hello:?}"
    );
    assert!(
        log.contains("ffmpeg") && log.contains("/nonexistent/ffmpeg"),
        "{log}"
    );
    let plugin = scratch.install(&args);
    let error = PluginRunner::default()
        .probe(&plugin, &file(&fixture("clip.mkv")))
        .unwrap_err();
    assert!(
        matches!(
            error,
            PlatformError::PluginLacks {
                capability: Capability::Probe,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_program_that_is_not_ffmpeg_is_not_used() {
    let scratch = Scratch::new();
    // Both tools are imposters, so the test does not depend on what this machine has installed.
    let mut args = Vec::new();
    for (flag, name) in [("--ffmpeg", "ffmpeg"), ("--ffprobe", "ffprobe")] {
        let imposter = scratch.path(name);
        std::fs::write(&imposter, "#!/bin/sh\necho 'not what you think'\n").unwrap();
        let mut perms = std::fs::metadata(&imposter).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&imposter, perms).unwrap();
        args.extend([flag.to_owned(), imposter.display().to_string()]);
    }
    let (hello, log) = greeting(&args);
    assert!(hello.provides.is_empty());
    assert!(log.contains("cannot be used"), "{log}");
}

#[test]
fn the_runner_hands_back_the_greeting_so_a_host_offers_only_what_can_be_written() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let fake = scratch.fake_ffmpeg("exit 0");
    let plugin = scratch.install(&support::with_ffmpeg(&fake));
    let hello = PluginRunner::default().hello(&plugin).unwrap();
    assert_eq!(hello.targets, ["trim", "audio-copy", "flac"]);
    assert!(hello.provides.contains(&Capability::Export));

    let missing = scratch.install(&["--ffmpeg".to_owned(), "/nonexistent/ffmpeg".to_owned()]);
    let hello = PluginRunner::default().hello(&missing).unwrap();
    assert!(
        hello.targets.is_empty() && !hello.provides.contains(&Capability::Export),
        "a plugin with no ffmpeg offers nothing: {hello:?}"
    );
}
