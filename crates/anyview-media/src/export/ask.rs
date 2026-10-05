//! What a media export asks of the plugin that writes it, in the plugin's own words: one of its
//! export targets, the part of the recording to keep and the bitrate of a lossy target. Pure.

use crate::MediaError;
use anyview_core::{
    AudioTarget, Bitrate, ExportJob, FormatKind, MediaExportKind, MediaTime, StreamPick,
};

/// One request to a plugin's `export`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ask {
    /// The export target's name, as the plugin's manifest lists it.
    pub target: &'static str,
    /// Where the part to keep begins; the start when it is zero.
    pub from: MediaTime,
    /// Where it ends; the end of the recording when absent.
    pub to: Option<MediaTime>,
    /// The bitrate of a lossy target; the plugin's own default when absent.
    pub bitrate: Option<Bitrate>,
}

/// The plugin target that writes `kind`, or `None` for what the player writes (a frame).
pub fn target_of(kind: MediaExportKind) -> Option<&'static str> {
    match kind {
        MediaExportKind::FramePng
        | MediaExportKind::FrameJpeg
        | MediaExportKind::FrameWebp
        | MediaExportKind::FrameAvif
        | MediaExportKind::FrameTiff => None,
        MediaExportKind::Trim => Some("trim"),
        MediaExportKind::ExtractAudio => Some("audio-copy"),
        MediaExportKind::ToM4a => Some("m4a"),
        MediaExportKind::ToMp3 => Some("mp3"),
        MediaExportKind::ToFlac => Some("flac"),
        MediaExportKind::ToWav => Some("wav"),
        MediaExportKind::ToOpus => Some("opus"),
    }
}

/// The kinds of export a file of `file` offers, whoever can write them: a frame is a video's,
/// and the rest are of any recording.
pub fn offered_kinds(file: FormatKind) -> Vec<MediaExportKind> {
    use ds_core::word::Word;
    MediaExportKind::ALL
        .iter()
        .copied()
        .filter(|kind| match (target_of(*kind), file) {
            (None, FormatKind::Video) => true,
            (None, _) => false,
            (Some(_), _) => true,
        })
        .collect()
}

/// What `job` asks of the plugin, or `NotMedia` for a job that is not a recording's transcode.
pub fn ask_of(job: &ExportJob) -> Result<Ask, MediaError> {
    let ExportJob::Transcode {
        range,
        streams,
        audio,
        ..
    } = job
    else {
        return Err(MediaError::NotMedia);
    };
    let (target, bitrate) = match (streams, audio) {
        (StreamPick::Everything, _) => ("trim", None),
        (StreamPick::AudioOnly, AudioTarget::Copy) => ("audio-copy", None),
        (StreamPick::AudioOnly, AudioTarget::M4a(rate)) => ("m4a", Some(*rate)),
        (StreamPick::AudioOnly, AudioTarget::Mp3(rate)) => ("mp3", Some(*rate)),
        (StreamPick::AudioOnly, AudioTarget::Flac) => ("flac", None),
        (StreamPick::AudioOnly, AudioTarget::Wav) => ("wav", None),
        (StreamPick::AudioOnly, AudioTarget::Opus(rate)) => ("opus", Some(*rate)),
    };
    Ok(Ask {
        target,
        from: range.start(),
        to: range.end(),
        bitrate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FilePath, TimeRange};

    fn transcode(streams: StreamPick, audio: AudioTarget, range: TimeRange) -> ExportJob {
        ExportJob::Transcode {
            source: FilePath::new("/v/clip.mkv").unwrap(),
            range,
            streams,
            audio,
        }
    }

    #[test]
    fn each_job_asks_for_the_target_that_writes_it() {
        let rate = Bitrate::from_kbps(128);
        let cut = TimeRange::new(MediaTime::from_secs(1), Some(MediaTime::from_secs(2))).unwrap();
        let from_one = TimeRange::new(MediaTime::from_secs(1), None).unwrap();
        // name, job, target, range, bitrate
        type Row = (
            &'static str,
            ExportJob,
            &'static str,
            (u64, Option<u64>),
            Option<Bitrate>,
        );
        let cases: Vec<Row> = vec![
            (
                "a trim",
                transcode(StreamPick::Everything, AudioTarget::Copy, cut),
                "trim",
                (1_000_000, Some(2_000_000)),
                None,
            ),
            (
                "a trim with no end mark",
                transcode(StreamPick::Everything, AudioTarget::Copy, from_one),
                "trim",
                (1_000_000, None),
                None,
            ),
            (
                "the track as it is",
                transcode(StreamPick::AudioOnly, AudioTarget::Copy, TimeRange::WHOLE),
                "audio-copy",
                (0, None),
                None,
            ),
            (
                "mp3 carries its bitrate",
                transcode(
                    StreamPick::AudioOnly,
                    AudioTarget::Mp3(rate),
                    TimeRange::WHOLE,
                ),
                "mp3",
                (0, None),
                Some(rate),
            ),
            (
                "m4a carries its bitrate",
                transcode(
                    StreamPick::AudioOnly,
                    AudioTarget::M4a(rate),
                    TimeRange::WHOLE,
                ),
                "m4a",
                (0, None),
                Some(rate),
            ),
            (
                "opus carries its bitrate",
                transcode(
                    StreamPick::AudioOnly,
                    AudioTarget::Opus(rate),
                    TimeRange::WHOLE,
                ),
                "opus",
                (0, None),
                Some(rate),
            ),
            (
                "flac has none",
                transcode(StreamPick::AudioOnly, AudioTarget::Flac, TimeRange::WHOLE),
                "flac",
                (0, None),
                None,
            ),
            (
                "wav has none",
                transcode(StreamPick::AudioOnly, AudioTarget::Wav, TimeRange::WHOLE),
                "wav",
                (0, None),
                None,
            ),
        ];
        for (name, job, target, (from, to), bitrate) in cases {
            let ask = ask_of(&job).unwrap();
            assert_eq!(ask.target, target, "{name}");
            assert_eq!(ask.from.0, from, "{name}");
            assert_eq!(ask.to.map(|at| at.0), to, "{name}");
            assert_eq!(ask.bitrate, bitrate, "{name}");
        }
    }

    #[test]
    fn a_frame_is_not_a_plugin_job() {
        let shot = ExportJob::MpvScreenshot {
            target: anyview_core::RasterTarget::Png,
            subtitles: anyview_core::Subtitles::Omit,
        };
        assert_eq!(ask_of(&shot), Err(MediaError::NotMedia));
    }

    #[test]
    fn every_kind_that_is_not_a_frame_names_a_target_and_a_frame_is_only_a_videos() {
        use ds_core::word::Word;
        let audio = offered_kinds(FormatKind::Audio);
        let video = offered_kinds(FormatKind::Video);
        assert!(
            audio.iter().all(|kind| target_of(*kind).is_some()),
            "an audio file offers no frame: {audio:?}"
        );
        assert_eq!(
            video.len(),
            MediaExportKind::ALL.len(),
            "a video offers every kind"
        );
        assert_eq!(
            audio.len() + 5,
            video.len(),
            "five frame formats are the video's"
        );
        let names: Vec<&str> = MediaExportKind::ALL
            .iter()
            .filter_map(|kind| target_of(*kind))
            .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "no two kinds share a target");
    }
}
