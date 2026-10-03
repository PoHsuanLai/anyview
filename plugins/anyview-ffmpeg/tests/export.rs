//! Export through the host's `PluginRunner`: every target probed back with ffprobe, the keyframe
//! alignment of a trim, progress, refusing to overwrite, and cancel with a stand-in ffmpeg.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::work::Stop;
use anyview_platform::{PlatformError, PluginRunner, Timeouts};
use anyview_plugin_protocol::{ErrorCode, ExportRequest, MicroRange, Progress};
use std::path::Path;
use std::time::Duration;
use support::{Scratch, codec_and_duration, ffprobe, fixture};

fn request(input: &Path, output: &Path, target: &str) -> ExportRequest {
    ExportRequest {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        target: target.to_owned(),
        range: None,
        stream: None,
        bitrate: None,
    }
}

fn run(scratch: &Scratch, request: &ExportRequest) -> (Result<(), PlatformError>, Vec<Progress>) {
    let plugin = scratch.install(&[]);
    let mut seen = Vec::new();
    let done = PluginRunner::default().export(&plugin, request, &Stop::new(), |p| seen.push(p));
    (done.map(|_| ()), seen)
}

#[test]
fn each_target_writes_a_file_of_its_codec_and_the_length_of_the_source() {
    require_ffmpeg!();
    // target, extension, codec ffprobe reports, source
    const CASES: &[(&str, &str, &str, &str)] = &[
        ("m4a", "m4a", "aac", "tone.flac"),
        ("mp3", "mp3", "mp3", "tone.flac"),
        ("flac", "flac", "flac", "tone.flac"),
        ("wav", "wav", "pcm_s16le", "tone.flac"),
        ("opus", "opus", "opus", "tone.flac"),
        ("audio-copy", "flac", "flac", "tone.flac"),
        ("mp3", "mp3", "mp3", "clip.mkv"),
        ("audio-copy", "mka", "vorbis", "clip.mkv"),
    ];
    for (target, extension, codec, source) in CASES {
        let scratch = Scratch::new();
        let output = scratch.path(&format!("out.{extension}"));
        let asked = request(&fixture(source), &output, target);
        let (done, _) = run(&scratch, &asked);
        done.unwrap_or_else(|error| panic!("{target} of {source}: {error:?}"));
        let (got, seconds) = codec_and_duration(&output);
        assert_eq!(got, *codec, "{target} of {source}");
        let whole = if *source == "tone.flac" { 2.0 } else { 3.0 };
        assert!(
            (seconds - whole).abs() < 0.25,
            "{target} of {source} lasts {seconds}, not {whole}"
        );
        assert!(
            !scratch.path(&format!(".part-out.{extension}")).exists(),
            "{target}: no partial file is left"
        );
    }
}

#[test]
fn a_bitrate_in_the_request_is_the_one_encoded() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let output = scratch.path("out.mp3");
    let mut asked = request(&fixture("tone.flac"), &output, "mp3");
    asked.bitrate = Some(64_000);
    run(&scratch, &asked).0.unwrap();
    let json = ffprobe(&output, &["-show_streams"]);
    let bits: u64 = json["streams"][0]["bit_rate"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((60_000..=68_000).contains(&bits), "{bits}");
}

#[test]
fn an_audio_range_keeps_only_that_span() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let output = scratch.path("part.flac");
    let mut asked = request(&fixture("tone.flac"), &output, "flac");
    asked.range = Some(MicroRange {
        start: 500_000,
        end: 1_500_000,
    });
    run(&scratch, &asked).0.unwrap();
    let (_, seconds) = codec_and_duration(&output);
    assert!((seconds - 1.0).abs() < 0.1, "{seconds}");
}

/// The presentation time in microseconds and whether it is a keyframe, of each video packet.
fn video_packets(path: &Path) -> Vec<(i64, bool)> {
    let json = ffprobe(
        path,
        &[
            "-select_streams",
            "v",
            "-show_entries",
            "packet=pts_time,flags",
        ],
    );
    json["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|packet| {
            let seconds: f64 = packet["pts_time"].as_str().unwrap().parse().unwrap();
            let key = packet["flags"].as_str().unwrap().contains('K');
            ((seconds * 1_000_000.0).round() as i64, key)
        })
        .collect()
}

#[test]
fn a_trim_begins_on_a_keyframe_at_time_zero_and_ends_at_the_end_asked() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let output = scratch.path("cut.mkv");
    let mut asked = request(&fixture("clip.mkv"), &output, "trim");
    asked.range = Some(MicroRange {
        start: 1_500_000,
        end: 2_500_000,
    });
    let (done, _) = run(&scratch, &asked);
    done.unwrap();
    let packets = video_packets(&output);
    let (first, first_is_key) = packets[0];
    assert!(first_is_key, "the cut starts on a keyframe");
    assert!(
        first.abs() < 50_000,
        "the first picture is at time zero: {first}"
    );
    let last = packets.iter().map(|(time, _)| *time).max().unwrap();
    assert!(
        (1_100_000..1_300_000).contains(&last),
        "the last picture is before 2.5 s: {last}"
    );
    assert_eq!(
        packets.iter().filter(|(_, key)| *key).count(),
        2,
        "the keyframes at 1.232 s and 2.432 s"
    );
    // Everything is copied: video, both audio tracks and the subtitles, and the chapters.
    let json = ffprobe(&output, &["-show_streams", "-show_chapters"]);
    let kinds: Vec<&str> = json["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["codec_type"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["video", "audio", "audio", "subtitle"]);
    assert!(
        !json["chapters"].as_array().unwrap().is_empty(),
        "a chapter overlaps the cut"
    );
}

#[test]
fn a_trim_of_the_whole_recording_keeps_every_picture() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let output = scratch.path("same.mkv");
    run(&scratch, &request(&fixture("clip.mkv"), &output, "trim"))
        .0
        .unwrap();
    assert_eq!(
        video_packets(&output).len(),
        video_packets(&fixture("clip.mkv")).len()
    );
}

#[test]
fn an_export_reports_progress_up_to_the_whole() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let output = scratch.path("p.opus");
    let (done, seen) = run(&scratch, &request(&fixture("tone.flac"), &output, "opus"));
    done.unwrap();
    assert!(!seen.is_empty());
    assert!(
        seen.windows(2).all(|pair| pair[0].done <= pair[1].done),
        "in order: {seen:?}"
    );
    let last = seen.last().unwrap();
    assert_eq!((last.done, last.total), (2_000_000, 2_000_000), "{seen:?}");
}

#[test]
fn an_export_never_overwrites_and_says_why() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let output = scratch.path("taken.flac");
    std::fs::write(&output, b"mine").unwrap();
    let (done, seen) = run(&scratch, &request(&fixture("tone.flac"), &output, "flac"));
    let error = done.unwrap_err();
    assert!(
        matches!(&error, PlatformError::PluginFailed { message, .. } if message.contains("already exists")),
        "{error:?}"
    );
    assert!(seen.is_empty());
    assert_eq!(std::fs::read(&output).unwrap(), b"mine");
}

#[test]
fn a_target_nobody_wrote_and_a_recording_with_no_sound_are_unsupported() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let (done, _) = run(
        &scratch,
        &request(&fixture("tone.flac"), &scratch.path("o.ogg"), "ogg"),
    );
    assert!(
        matches!(
            done,
            Err(PlatformError::PluginFailed {
                code: ErrorCode::Unsupported,
                ..
            })
        ),
        "{done:?}"
    );
    // A recording with no sound has no audio to extract.
    let silent = scratch.path("silent.mp4");
    let made = std::process::Command::new(support::on_path("ffmpeg").unwrap())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=16x16:d=1",
            "-c:v",
            "mpeg4",
        ])
        .arg(&silent)
        .status()
        .unwrap();
    assert!(made.success());
    let (done, _) = run(&scratch, &request(&silent, &scratch.path("o.mp3"), "mp3"));
    assert!(
        matches!(
            done,
            Err(PlatformError::PluginFailed {
                code: ErrorCode::Unsupported,
                ..
            })
        ),
        "{done:?}"
    );
}

fn quick() -> PluginRunner {
    PluginRunner::new(Timeouts {
        hello: Duration::from_secs(5),
        silence: Duration::from_secs(10),
        cancel_grace: Duration::from_secs(3),
    })
}

/// Whether the process `pid` is still there.
fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[test]
fn cancel_kills_ffmpeg_and_removes_the_partial_file() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let pidfile = scratch.path("ffmpeg.pid");
    let fake = scratch.fake_ffmpeg(&format!(
        "echo $$ > {pid}\necho partial > \"$out\"\ni=0\nwhile true; do i=$((i+100000)); echo out_time_us=$i; sleep 0.05; done",
        pid = pidfile.display()
    ));
    let plugin = scratch.install(&support::with_ffmpeg(&fake));
    let output = scratch.path("never.flac");
    let asked = request(&fixture("tone.flac"), &output, "flac");
    let stop = Stop::new();
    let raise = stop.clone();
    let mut steps = 0;
    let error = quick()
        .export(&plugin, &asked, &stop, |_| {
            steps += 1;
            if steps == 2 {
                raise.request();
            }
        })
        .unwrap_err();
    assert_eq!(
        error,
        PlatformError::PluginCancelled {
            plugin: "ffmpeg".to_owned()
        }
    );
    assert!(
        (2..15).contains(&steps),
        "it stopped early, after {steps} steps"
    );
    assert!(!output.exists());
    assert!(
        !scratch.path(".part-never.flac").exists(),
        "the partial file is gone"
    );
    let pid: u32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // The plugin has exited, and it killed ffmpeg before it did.
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while alive(pid) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(pid), "ffmpeg (pid {pid}) was killed");
}

#[test]
fn a_failing_ffmpeg_is_an_error_with_what_it_said_and_leaves_no_file() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let fake = scratch
        .fake_ffmpeg("echo partial > \"$out\"\necho 'Error: the disk is on fire' >&2\nexit 1");
    let plugin = scratch.install(&support::with_ffmpeg(&fake));
    let output = scratch.path("bad.flac");
    let error = quick()
        .export(
            &plugin,
            &request(&fixture("tone.flac"), &output, "flac"),
            &Stop::new(),
            |_| {},
        )
        .unwrap_err();
    assert!(
        matches!(&error, PlatformError::PluginFailed { code: ErrorCode::Failed, message, .. }
            if message.contains("the disk is on fire")),
        "{error:?}"
    );
    assert!(!output.exists());
    assert!(!scratch.path(".part-bad.flac").exists());
}

#[test]
fn a_target_the_encoders_lack_is_refused_before_ffmpeg_is_asked() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    // The stand-in lists FLAC alone, so MP3 has no encoder here.
    let fake = scratch.fake_ffmpeg("echo should-not-run > \"$out\"");
    let plugin = scratch.install(&support::with_ffmpeg(&fake));
    let output = scratch.path("o.mp3");
    let error = quick()
        .export(
            &plugin,
            &request(&fixture("tone.flac"), &output, "mp3"),
            &Stop::new(),
            |_| {},
        )
        .unwrap_err();
    assert!(
        matches!(
            &error,
            PlatformError::PluginFailed {
                code: ErrorCode::Unsupported,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!output.exists());
}

#[test]
fn a_host_that_gives_up_on_a_silent_export_kills_ffmpeg_with_the_plugin() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let pidfile = scratch.path("ffmpeg.pid");
    let fake = scratch.fake_ffmpeg(&format!("echo $$ > {}\nsleep 1000", pidfile.display()));
    let plugin = scratch.install(&support::with_ffmpeg(&fake));
    let impatient = PluginRunner::new(Timeouts {
        hello: Duration::from_secs(5),
        silence: Duration::from_millis(600),
        cancel_grace: Duration::from_millis(300),
    });
    let output = scratch.path("never.flac");
    let error = impatient
        .export(
            &plugin,
            &request(&fixture("tone.flac"), &output, "flac"),
            &Stop::new(),
            |_| {},
        )
        .unwrap_err();
    assert!(
        matches!(error, PlatformError::PluginSilent { .. }),
        "{error:?}"
    );
    let pid: u32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while alive(pid) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(pid), "ffmpeg (pid {pid}) died with the plugin");
}
