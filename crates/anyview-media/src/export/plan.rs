//! A person's choice of media export as the jobs that carry it out. Pure: the planner reads no
//! file, so every choice has a row in the tests.

use anyview_core::{ExportJob, FilePath, MediaExport, StreamPick, Subtitles, TimeRange};

/// The jobs `choice` makes of the recording `source`. A frame is the player's to take (the binary
/// carries `MpvScreenshot` out through it); a trim and an audio export are the FFmpeg plugin's.
#[must_use]
pub fn plan_export(source: &FilePath, choice: MediaExport) -> Vec<ExportJob> {
    let transcode = |range, streams, audio| ExportJob::Transcode {
        source: source.clone(),
        range,
        streams,
        audio,
    };
    match choice {
        MediaExport::CurrentFrame(target) => vec![ExportJob::MpvScreenshot {
            target,
            subtitles: Subtitles::Omit,
        }],
        MediaExport::Trim(range) => vec![transcode(
            range,
            StreamPick::Everything,
            anyview_core::AudioTarget::Copy,
        )],
        MediaExport::AudioOnly(audio) => {
            vec![transcode(TimeRange::WHOLE, StreamPick::AudioOnly, audio)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{AudioTarget, MediaTime, RasterTarget};

    #[test]
    fn each_choice_becomes_the_jobs_that_carry_it_out() {
        let source = FilePath::new("/v/clip.mkv").unwrap();
        let range = TimeRange::new(MediaTime::from_secs(1), Some(MediaTime::from_secs(2))).unwrap();
        let mp3 = AudioTarget::Mp3(AudioTarget::default_bitrate());
        let cases: Vec<(&str, MediaExport, ExportJob)> = vec![
            (
                "a frame goes to the player",
                MediaExport::CurrentFrame(RasterTarget::Png),
                ExportJob::MpvScreenshot {
                    target: RasterTarget::Png,
                    subtitles: Subtitles::Omit,
                },
            ),
            (
                "a trim copies every stream over its range",
                MediaExport::Trim(range),
                ExportJob::Transcode {
                    source: source.clone(),
                    range,
                    streams: StreamPick::Everything,
                    audio: AudioTarget::Copy,
                },
            ),
            (
                "extracted audio copies the track",
                MediaExport::AudioOnly(AudioTarget::Copy),
                ExportJob::Transcode {
                    source: source.clone(),
                    range: TimeRange::WHOLE,
                    streams: StreamPick::AudioOnly,
                    audio: AudioTarget::Copy,
                },
            ),
            (
                "a conversion re-encodes the audio",
                MediaExport::AudioOnly(mp3),
                ExportJob::Transcode {
                    source: source.clone(),
                    range: TimeRange::WHOLE,
                    streams: StreamPick::AudioOnly,
                    audio: mp3,
                },
            ),
        ];
        for (name, choice, want) in cases {
            assert_eq!(plan_export(&source, choice), vec![want], "{name}");
        }
    }
}
