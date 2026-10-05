//! Recordings through the plugins, with the real FFmpeg plugin built from this workspace: the
//! facts and tags it reads, the exports it offers and writes (a cut, a track, a conversion) beside
//! the file with progress and a stop that removes what was written, and what the viewer does
//! when no plugin is installed. A machine with no FFmpeg skips what needs it, saying so.

#![allow(clippy::unwrap_used)]

mod support;

use anyview::host::{Outcome, Task};
use anyview::media::{ExportEnd, MediaHub, MediaPlugins, Playing};
use anyview::runtime::PoolSize;
use anyview::seam::{NoticeWaker, Workforce};
use anyview_core::{
    AudioTarget, ExportJob, FactLabel, FilePath, MediaExport, MediaExportKind, MediaTime,
    StreamPick, TimeRange,
};
use anyview_media::{AudioDriver, ExportProgress, ExportRequest};
use anyview_platform::testing::FakeMediaSession;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use support::{copy_into, desktop, media_fixture, on_path, plugins, plugins_with_args, probed};
use tokio::runtime::Runtime;

struct Rig {
    runtime: Runtime,
    hub: MediaHub,
    workforce: Arc<Workforce>,
    plugins: Arc<MediaPlugins>,
}

fn rig(plugins: Arc<MediaPlugins>) -> Rig {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let hub = MediaHub::start(
        runtime.handle(),
        || std::future::ready(FakeMediaSession::new()),
        None,
        AudioDriver::Null,
        Arc::clone(&plugins),
    );
    let workforce = Arc::new(
        Workforce::start(
            PoolSize::exactly(std::num::NonZeroUsize::new(2).unwrap()),
            NoticeWaker::default(),
        )
        .unwrap(),
    );
    Rig {
        runtime,
        hub,
        workforce,
        plugins,
    }
}

/// The recording's length in milliseconds, as ffprobe reads it.
fn millis(path: &Path) -> u64 {
    let out = Command::new(on_path("ffprobe").unwrap())
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .unwrap();
    let seconds: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    (seconds * 1000.0).round() as u64
}

fn export(rig: &Rig, dir: &Path, file: &FilePath, choice: MediaExport) -> Outcome {
    let hosting = desktop(&rig.runtime, &rig.hub, &rig.workforce, dir);
    rig.runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(file),
            choice,
        }))
        .unwrap()
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_cut_a_track_and_a_conversion_are_written_beside_the_recording() {
    let Some(plugins) = plugins(false, true) else {
        return;
    };
    let rig = rig(plugins);
    let dir = tempfile::tempdir().unwrap();
    let clip = copy_into(dir.path(), "clip.mkv");
    let trim = MediaExport::Trim(
        TimeRange::new(
            MediaTime::from_millis(1500),
            Some(MediaTime::from_millis(2500)),
        )
        .unwrap(),
    );
    assert_eq!(export(&rig, dir.path(), &clip, trim), Outcome::Done);
    let cut = dir.path().join("clip trimmed.mkv");
    assert!(cut.exists(), "{:?}", listing(dir.path()));
    let length = millis(&cut);
    assert!(
        (900..=1800).contains(&length),
        "a keyframe-aligned cut about a second long: {length} ms"
    );
    assert!(
        !listing(dir.path()).iter().any(|name| name.starts_with('.')),
        "no partial file is left: {:?}",
        listing(dir.path())
    );

    // The track as it is, named by its codec (vorbis lives in Ogg).
    let track = MediaExport::AudioOnly(AudioTarget::Copy);
    assert_eq!(export(&rig, dir.path(), &clip, track), Outcome::Done);
    assert!(
        dir.path().join("clip.ogg").exists(),
        "{:?}",
        listing(dir.path())
    );

    let tone = copy_into(dir.path(), "tone.flac");
    let wav = MediaExport::AudioOnly(AudioTarget::Wav);
    assert_eq!(export(&rig, dir.path(), &tone, wav), Outcome::Done);
    assert!(dir.path().join("tone.wav").exists());
    let wav_length = millis(&dir.path().join("tone.wav"));
    assert!((1900..=2100).contains(&wav_length), "{wav_length} ms");

    // A name that is taken is never overwritten.
    assert_eq!(export(&rig, dir.path(), &tone, wav), Outcome::Done);
    assert!(dir.path().join("tone 2.wav").exists());
}

#[test]
fn a_trim_marked_only_at_its_start_runs_to_the_end_of_the_recording() {
    let Some(plugins) = plugins(false, true) else {
        return;
    };
    let rig = rig(plugins);
    let dir = tempfile::tempdir().unwrap();
    let clip = copy_into(dir.path(), "clip.mkv");
    let from_one_and_a_half =
        MediaExport::Trim(TimeRange::new(MediaTime::from_millis(1500), None).unwrap());
    assert_eq!(
        export(&rig, dir.path(), &clip, from_one_and_a_half),
        Outcome::Done
    );
    let length = millis(&dir.path().join("clip trimmed.mkv"));
    assert!(
        (1500..=2000).contains(&length),
        "from the keyframe at 1.2 s to the end of 3 s: {length} ms"
    );
}

/// A request to convert the tone to FLAC next to a copy of it in `dir`.
fn conversion(dir: &Path, progress: anyview_media::ProgressSink) -> ExportRequest {
    let source = copy_into(dir, "tone.flac");
    ExportRequest {
        job: ExportJob::Transcode {
            source,
            range: TimeRange::WHOLE,
            streams: StreamPick::AudioOnly,
            audio: AudioTarget::Wav,
        },
        to: FilePath::new(dir.join("out.wav")).unwrap(),
        progress,
    }
}

#[test]
fn an_export_reports_progress_that_never_goes_back_and_ends_where_the_work_does() {
    let Some(plugins) = plugins(false, true) else {
        return;
    };
    let rig = rig(plugins);
    let dir = tempfile::tempdir().unwrap();
    let seen: Arc<Mutex<Vec<ExportProgress>>> = Arc::default();
    let sink = Arc::clone(&seen);
    let request = conversion(
        dir.path(),
        Arc::new(move |progress| sink.lock().unwrap().push(progress)),
    );
    let subject = anyview_plugin::Subject {
        kind: anyview_core::FormatKind::Audio,
        mime: None,
    };
    let anyview::media::WriteRoute::Ready(tool) = rig.plugins.writer(&subject) else {
        panic!("the FFmpeg plugin is installed");
    };
    let ended = rig
        .runtime
        .block_on(rig.workforce.exports().submit(tool, request).ended());
    assert!(matches!(ended, ExportEnd::Written(_)), "{ended:?}");
    let seen = seen.lock().unwrap();
    assert!(!seen.is_empty(), "progress was reported");
    assert!(
        seen.windows(2).all(|pair| pair[0].done <= pair[1].done),
        "never backwards: {seen:?}"
    );
    let last = seen.last().unwrap();
    assert_eq!(
        last.of.map(|length| length.0),
        Some(last.done),
        "the last report says the work is done: {last:?}"
    );
}

#[test]
fn a_stopped_export_is_stopped_and_leaves_nothing_behind() {
    // An FFmpeg that starts writing and then takes its time, so there is a run to stop.
    use std::os::unix::fs::PermissionsExt;
    let tools = tempfile::tempdir().unwrap();
    let slow = tools.path().join("ffmpeg-slow");
    std::fs::write(
        &slow,
        "#!/bin/sh\ncase \"$1\" in\n-version) echo 'ffmpeg version 8.1 fake'; exit 0;;\n\
         -hide_banner) printf 'Encoders:\\n ------\\n A....D flac x\\n'; exit 0;;\nesac\n\
         for out; do :; done\necho partial > \"$out\"\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
    let args = ["--ffmpeg".to_owned(), slow.display().to_string()];
    let Some(plugins) = plugins_with_args(false, true, &args) else {
        return;
    };
    let rig = rig(plugins);
    let dir = tempfile::tempdir().unwrap();
    let mut request = conversion(dir.path(), Arc::new(|_| {}));
    request.job = ExportJob::Transcode {
        source: copy_into(dir.path(), "tone.flac"),
        range: TimeRange::WHOLE,
        streams: StreamPick::AudioOnly,
        audio: AudioTarget::Flac,
    };
    let before = listing(dir.path());
    let subject = anyview_plugin::Subject {
        kind: anyview_core::FormatKind::Audio,
        mime: None,
    };
    let anyview::media::WriteRoute::Ready(tool) = rig.plugins.writer(&subject) else {
        panic!("the FFmpeg plugin is installed");
    };
    let handle = rig.workforce.exports().submit(tool, request);
    // The partial file shows up, then the stop comes.
    let has_partial = || {
        std::fs::read_dir(dir.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".part-")
        })
    };
    let started = std::time::Instant::now();
    while !has_partial() && started.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        has_partial(),
        "the plugin began writing: {:?}",
        listing(dir.path())
    );
    handle.stop();
    let ended = rig.runtime.block_on(handle.ended());
    assert!(
        matches!(ended, ExportEnd::Failed(anyview_media::MediaError::Stopped)),
        "{ended:?}"
    );
    assert_eq!(
        listing(dir.path()),
        before,
        "neither the file nor the partial one is left"
    );
}

#[test]
fn with_no_plugin_installed_an_export_says_which_package_writes_it() {
    let rig = rig(Arc::new(MediaPlugins::default()));
    let dir = tempfile::tempdir().unwrap();
    let tone = copy_into(dir.path(), "tone.flac");
    let outcome = export(
        &rig,
        dir.path(),
        &tone,
        MediaExport::AudioOnly(AudioTarget::Wav),
    );
    let Outcome::Failed(message) = outcome else {
        panic!("an export with no writer fails: {outcome:?}");
    };
    assert!(message.contains("anyview-ffmpeg"), "{message}");
    assert_eq!(listing(dir.path()), ["tone.flac"], "nothing was written");
}

/// A program that stands in for ffmpeg and can encode FLAC only.
fn flac_only_ffmpeg(dir: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("ffmpeg-flac-only");
    std::fs::write(
        &path,
        "#!/bin/sh\ncase \"$1\" in\n-version) echo 'ffmpeg version 8.1 fake'; exit 0;;\n\
         -hide_banner) printf 'Encoders:\\n ------\\n A....D flac x\\n'; exit 0;;\nesac\nexit 0\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.display().to_string()
}

fn kinds_of(plugins: &MediaPlugins, file: &str, playing: Playing) -> Vec<MediaExportKind> {
    let probed = probed(&media_fixture(file));
    plugins.offer(&probed.sniffed, playing).kinds().to_vec()
}

#[test]
fn the_sheet_offers_what_the_machine_can_write_and_only_what_fits_the_file() {
    let Some(all) = plugins(false, true) else {
        return;
    };
    let audio = kinds_of(&all, "tone.flac", Playing::No);
    assert_eq!(
        audio,
        [
            MediaExportKind::Trim,
            MediaExportKind::ExtractAudio,
            MediaExportKind::ToM4a,
            MediaExportKind::ToMp3,
            MediaExportKind::ToFlac,
            MediaExportKind::ToWav,
            MediaExportKind::ToOpus,
        ],
        "an audio file has no frame to save"
    );
    let frames = [
        MediaExportKind::FramePng,
        MediaExportKind::FrameJpeg,
        MediaExportKind::FrameWebp,
        MediaExportKind::FrameAvif,
        MediaExportKind::FrameTiff,
    ];
    let shown = kinds_of(&all, "clip.mkv", Playing::Yes);
    assert!(
        frames.iter().all(|frame| shown.contains(frame)),
        "a video the player shows offers its frame: {shown:?}"
    );
    let hidden = kinds_of(&all, "clip.mkv", Playing::No);
    assert!(
        frames.iter().all(|frame| !hidden.contains(frame)),
        "without a player the frame is not on offer: {hidden:?}"
    );
    assert!(
        all.offer(&probed(&media_fixture("tone.flac")).sniffed, Playing::No)
            .needs()
            .is_none(),
        "nothing is missing"
    );

    // An FFmpeg that can encode only FLAC offers only what it can write.
    let dir = tempfile::tempdir().unwrap();
    let args = ["--ffmpeg".to_owned(), flac_only_ffmpeg(dir.path())];
    let some = plugins_with_args(false, true, &args).unwrap();
    assert_eq!(
        kinds_of(&some, "tone.flac", Playing::No),
        [
            MediaExportKind::Trim,
            MediaExportKind::ExtractAudio,
            MediaExportKind::ToFlac,
        ]
    );
}

#[test]
fn with_no_ffmpeg_plugin_nothing_is_offered_but_the_frame_and_the_package_is_named() {
    let none = MediaPlugins::default();
    let offer = none.offer(&probed(&media_fixture("tone.flac")).sniffed, Playing::No);
    assert!(offer.kinds().is_empty(), "{:?}", offer.kinds());
    let needs = offer.needs().expect("the missing package is named");
    assert_eq!(needs.label, FactLabel::Needs);
    assert!(needs.value.as_str().contains("anyview-ffmpeg"), "{needs:?}");

    let video = none.offer(&probed(&media_fixture("clip.mkv")).sniffed, Playing::Yes);
    assert_eq!(
        video.kinds().first(),
        Some(&MediaExportKind::FramePng),
        "a frame is the player's, not the plugin's"
    );
    assert!(
        video
            .kinds()
            .iter()
            .all(|kind| anyview_media::target_of(*kind).is_none()),
        "{:?}",
        video.kinds()
    );
}

#[test]
fn facts_come_from_the_ffmpeg_plugin_and_else_from_the_header_in_pure_rust() {
    let clip = probed(&media_fixture("clip.mkv"));
    // No plugin: the header reader says what it can, and the title is the file's own.
    let header = MediaPlugins::default().reading(&clip.source, &clip.sniffed);
    assert!(
        header
            .facts
            .rows()
            .iter()
            .any(|row| row.label == FactLabel::Duration),
        "the header has a length: {:?}",
        header.facts
    );
    assert!(
        header.facts.rows().iter().all(|row| !matches!(
            row.label,
            FactLabel::Kind | FactLabel::Size | FactLabel::Modified | FactLabel::Needs
        )),
        "the rows the window adds itself are left out: {:?}",
        header.facts
    );
    let Some(with) = plugins(false, true) else {
        return;
    };
    let read = with.reading(&clip.source, &clip.sniffed);
    let value = |label: FactLabel| {
        read.facts
            .value(label)
            .map(|value| value.as_str().to_owned())
    };
    assert_eq!(value(FactLabel::Duration).as_deref(), Some("0:03"));
    assert_eq!(value(FactLabel::Dimensions).as_deref(), Some("64 × 48"));
    assert_eq!(value(FactLabel::Codec).as_deref(), Some("mpeg4"));
    assert_eq!(value(FactLabel::AudioCodec).as_deref(), Some("vorbis"));
}
