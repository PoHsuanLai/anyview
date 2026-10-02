//! What only libav can see of a cut: the packets it wrote.

#![allow(clippy::unwrap_used)]

use super::{ExportBackend, ExportRequest};
use anyview_core::work::{Backend, Stop};
use anyview_core::{AudioTarget, ExportJob, FilePath, MediaTime, StreamPick, TimeRange};
use ff::media::Type;
use ffmpeg_next as ff;
use std::path::Path;
use std::sync::Arc;

fn video_packets(path: &Path) -> Vec<(i64, bool)> {
    ff::init().unwrap();
    let mut input = ff::format::input(path.to_str().unwrap()).unwrap();
    let index = input.streams().best(Type::Video).unwrap().index();
    let base = input.stream(index).unwrap().time_base();
    input
        .packets()
        .filter(|(stream, _)| stream.index() == index)
        .map(|(_, packet)| {
            let pts = packet.pts().or(packet.dts()).unwrap();
            let micros =
                pts * i64::from(base.numerator()) * 1_000_000 / i64::from(base.denominator());
            (micros, packet.is_key())
        })
        .collect()
}

#[test]
fn a_trim_begins_on_a_keyframe_at_time_zero_and_ends_at_the_end_asked() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mkv");
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("cut.mkv");
    let range = TimeRange::new(
        MediaTime::from_millis(1500),
        Some(MediaTime::from_millis(2500)),
    )
    .unwrap();
    let request = ExportRequest {
        job: ExportJob::Transcode {
            source: FilePath::new(&source).unwrap(),
            range,
            streams: StreamPick::Everything,
            audio: AudioTarget::Copy,
        },
        to: FilePath::new(&to).unwrap(),
        progress: Arc::new(|_| {}),
    };
    ExportBackend::run(&(), &mut (), request, &Stop::new()).unwrap();
    let packets = video_packets(&to);
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
}
