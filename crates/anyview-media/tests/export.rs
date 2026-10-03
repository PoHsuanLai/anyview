//! The exports, each checked by probing the file it wrote.

#![allow(clippy::unwrap_used)]

mod libav;

use anyview_core::work::Stop;
use anyview_core::{AudioTarget, Bitrate, MediaTime, StreamKind, StreamPick, TimeRange};
use anyview_media::{AudioFormat, Encoder, Encoders, ExportProgress, MediaError};
use libav::{file, fixture, probed, run, seconds, tagged_wav, transcode};

fn kinds(probe: &anyview_media::MediaProbe) -> Vec<StreamKind> {
    probe.tracks.iter().map(|track| track.kind).collect()
}

#[test]
fn a_trim_keeps_every_stream_and_starts_at_the_keyframe_before_the_time_asked() {
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("clip trimmed.mkv");
    let range = TimeRange::new(
        MediaTime::from_millis(1500),
        Some(MediaTime::from_millis(2500)),
    )
    .unwrap();
    let job = transcode(
        &fixture("clip.mkv"),
        range,
        StreamPick::Everything,
        AudioTarget::Copy,
    );
    let (done, _) = run(job, &to, &Stop::new());
    let report = done.unwrap();
    assert_eq!(report.path, file(&to));
    let out = probed(&to);
    // The keyframes are at 0.032, 1.232 and 2.432 s: the cut begins at the one before 1.5 s, so it
    // runs from 1.232 s to 2.5 s (1.268 s), not the 1.0 s that was asked for.
    let length = seconds(&out);
    assert!((1.15..1.40).contains(&length), "{length}");
    assert!((length - 1.0).abs() > 0.1, "{length}");
    assert_eq!(
        kinds(&out),
        vec![
            StreamKind::Video,
            StreamKind::Audio,
            StreamKind::Audio,
            StreamKind::Subtitles
        ]
    );
    assert_eq!(
        out.chapters.len(),
        1,
        "only the chapter that overlaps the cut stays"
    );
    assert_eq!(out.chapters[0].title, "Middle");
    assert_eq!(out.chapters[0].start.0, 0);
    assert!(!dir.path().join(".part-clip trimmed.mkv").exists());
}

#[test]
fn extracted_audio_is_the_track_in_a_container_of_its_codec() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("clip.mkv", "vorbis", "ogg"),
        ("cover.mp3", "mp3", "mp3"),
        ("tone.flac", "flac", "flac"),
    ];
    for (source, codec, extension) in cases {
        let to = dir.path().join(format!("{source}.{extension}"));
        let job = transcode(
            &fixture(source),
            TimeRange::WHOLE,
            StreamPick::AudioOnly,
            AudioTarget::Copy,
        );
        let (done, _) = run(job, &to, &Stop::new());
        done.unwrap_or_else(|error| panic!("{source}: {error}"));
        let out = probed(&to);
        assert_eq!(
            kinds(&out),
            vec![StreamKind::Audio],
            "{source} has audio only"
        );
        assert_eq!(out.audio.as_ref().unwrap().codec, codec, "{source}");
        let want = if source == "cover.mp3" {
            2.0
        } else {
            seconds(&probed(&fixture(source)))
        };
        assert!(
            (seconds(&out) - want).abs() < 0.15,
            "{source}: {}",
            seconds(&out)
        );
    }
}

fn conversions() -> Vec<(AudioFormat, AudioTarget, &'static str, &'static str)> {
    let rate = Bitrate::from_kbps(96);
    vec![
        (AudioFormat::M4a, AudioTarget::M4a(rate), "m4a", "aac"),
        (AudioFormat::Mp3, AudioTarget::Mp3(rate), "mp3", "mp3"),
        (AudioFormat::Flac, AudioTarget::Flac, "flac", "flac"),
        (AudioFormat::Wav, AudioTarget::Wav, "wav", "pcm_s16le"),
        (AudioFormat::Opus, AudioTarget::Opus(rate), "opus", "opus"),
    ]
}

#[test]
fn every_conversion_the_system_can_encode_writes_the_right_codec_and_length() {
    let encoders = Encoders::detect().unwrap();
    eprintln!("encoders: {encoders:?}; missing: {:?}", encoders.missing());
    let dir = tempfile::tempdir().unwrap();
    for (format, target, extension, codec) in conversions() {
        let to = dir.path().join(format!("tone.{extension}"));
        let job = transcode(
            &fixture("tone.flac"),
            TimeRange::WHOLE,
            StreamPick::AudioOnly,
            target,
        );
        let (done, reports) = run(job, &to, &Stop::new());
        match encoders.encoder(format) {
            Encoder::Missing => {
                eprintln!("skipped {format:?}: this libav has no encoder");
                assert_eq!(done, Err(MediaError::EncoderMissing(ds_label(format))));
                assert!(!to.exists());
            }
            Encoder::Available(name) => {
                done.unwrap_or_else(|error| panic!("{format:?} with {name}: {error}"));
                let out = probed(&to);
                assert_eq!(out.audio.as_ref().unwrap().codec, codec, "{format:?}");
                assert!(out.length.is_some(), "{format:?} has a length");
                assert!(
                    (seconds(&out) - 2.0).abs() < 0.1,
                    "{format:?}: {}",
                    seconds(&out)
                );
                assert_monotonic_and_complete(&reports, 2.0);
            }
        }
    }
}

fn ds_label(format: AudioFormat) -> &'static str {
    use ds_core::word::Word;
    format.label()
}

fn assert_monotonic_and_complete(reports: &[ExportProgress], length: f64) {
    assert!(reports.len() >= 2, "{} reports", reports.len());
    assert!(
        reports.windows(2).all(|pair| pair[0].done <= pair[1].done),
        "{reports:?}"
    );
    let last = reports.last().unwrap().done.0 as f64 / 1e6;
    assert!((last - length).abs() < 0.1, "ends at {last}");
    assert!(reports.iter().all(|report| report.of.is_some()));
}

#[test]
fn the_tags_of_the_source_are_carried_to_the_conversion() {
    let encoders = Encoders::detect().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("song.wav");
    std::fs::write(&source, tagged_wav()).unwrap();
    let from = probed(&source);
    assert_eq!(from.tags.title.as_deref(), Some("Song"));
    for (format, target, extension, _) in conversions() {
        if encoders.encoder(format) == Encoder::Missing {
            continue;
        }
        let to = dir.path().join(format!("out.{extension}"));
        let (done, _) = run(
            transcode(&source, TimeRange::WHOLE, StreamPick::AudioOnly, target),
            &to,
            &Stop::new(),
        );
        done.unwrap();
        let tags = probed(&to).tags;
        if format == AudioFormat::Wav
            || format == AudioFormat::Opus
            || format == AudioFormat::Mp3
            || format == AudioFormat::Flac
            || format == AudioFormat::M4a
        {
            assert_eq!(tags.title.as_deref(), Some("Song"), "{format:?} title");
            assert_eq!(tags.artist.as_deref(), Some("Band"), "{format:?} artist");
            assert_eq!(tags.album.as_deref(), Some("Record"), "{format:?} album");
        }
    }
}

#[test]
fn a_stop_raised_before_the_start_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("out.flac");
    let stop = Stop::new();
    stop.request();
    let job = transcode(
        &fixture("tone.flac"),
        TimeRange::WHOLE,
        StreamPick::AudioOnly,
        AudioTarget::Flac,
    );
    let (done, reports) = run(job, &to, &stop);
    assert_eq!(done, Err(MediaError::Stopped));
    assert!(reports.is_empty());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn a_stop_raised_mid_run_removes_the_partial_file() {
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("out.mkv");
    let stop = Stop::new();
    let raiser = stop.clone();
    let request = anyview_media::ExportRequest {
        job: transcode(
            &fixture("clip.mkv"),
            TimeRange::WHOLE,
            StreamPick::Everything,
            AudioTarget::Copy,
        ),
        to: file(&to),
        progress: std::sync::Arc::new(move |progress| {
            if progress.done.0 >= 500_000 {
                raiser.request();
            }
        }),
    };
    let done = <anyview_media::ExportBackend as anyview_core::work::Backend>::run(
        &(),
        &mut (),
        request,
        &stop,
    );
    assert_eq!(done, Err(MediaError::Stopped));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "nothing is left behind"
    );
}

#[test]
fn audio_of_a_recording_with_none_and_a_job_that_is_not_media_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let silent = dir.path().join("silent.png");
    std::fs::write(&silent, b"nothing to play").unwrap();
    let job = transcode(
        &silent,
        TimeRange::WHOLE,
        StreamPick::AudioOnly,
        AudioTarget::Copy,
    );
    let (done, _) = run(job, &dir.path().join("x.ogg"), &Stop::new());
    assert!(done.is_err());
    let shot = anyview_core::ExportJob::MpvScreenshot {
        target: anyview_core::RasterTarget::Png,
        subtitles: anyview_core::Subtitles::Omit,
    };
    let (done, _) = run(shot, &dir.path().join("x.png"), &Stop::new());
    assert_eq!(done, Err(MediaError::NotMedia));
}
