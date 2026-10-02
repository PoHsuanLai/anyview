//! Where an export is written: beside its source, under a name that is not taken. The caller says
//! which names are taken (a closure), so this reads no directory.

use crate::MediaError;
use crate::libav::file_path;
use crate::probe::MediaProbe;
use anyview_core::{AudioTarget, ExportJob, FilePath, MediaContainer, StreamPick};
use ds_core::word::Word;
use std::path::Path;

/// How many names are tried before giving up.
const TRIES: u32 = 10_000;

/// The path an export of `job` is written to: `<stem> trimmed.<ext>` for a trim, which keeps the
/// source's extension, and `<stem>.<ext>` for audio, with the extension its codec takes. A taken
/// name (`free` says `false`) gets ` 2`, ` 3`, … before the extension.
pub fn output_path(
    source: &FilePath,
    job: &ExportJob,
    probe: &MediaProbe,
    free: impl Fn(&Path) -> bool,
) -> Result<FilePath, MediaError> {
    let ExportJob::Transcode { streams, audio, .. } = job else {
        return Err(MediaError::NotMedia);
    };
    let stem = source
        .as_path()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("recording");
    let (base, extension) = match streams {
        StreamPick::Everything => (
            format!("{stem} trimmed"),
            source
                .as_path()
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_owned)
                .or_else(|| container_extension(probe.container))
                .unwrap_or_else(|| "mkv".to_owned()),
        ),
        StreamPick::AudioOnly => (stem.to_owned(), audio_extension(*audio, probe)),
    };
    let folder = source.parent().ok_or_else(|| MediaError::Io {
        op: "name",
        path: source.as_path().to_path_buf(),
        kind: std::io::ErrorKind::InvalidInput,
    })?;
    (1..=TRIES)
        .map(|n| match n {
            1 => format!("{base}.{extension}"),
            n => format!("{base} {n}.{extension}"),
        })
        .map(|name| folder.as_path().join(name))
        .find(|candidate| free(candidate))
        .ok_or_else(|| MediaError::Io {
            op: "name",
            path: folder.as_path().to_path_buf(),
            kind: std::io::ErrorKind::AlreadyExists,
        })
        .and_then(|path| file_path(&path))
}

/// The extension of the file audio is written as: fixed for a re-encode, and for a copy the one
/// the track's codec lives in.
fn audio_extension(target: AudioTarget, probe: &MediaProbe) -> String {
    match target {
        AudioTarget::Copy => {
            let codec = probe.audio.as_ref().map(|audio| audio.codec.as_str());
            match codec {
                Some("aac" | "alac") => "m4a".to_owned(),
                Some("mp3") => "mp3".to_owned(),
                Some("flac") => "flac".to_owned(),
                Some("vorbis") => "ogg".to_owned(),
                Some("opus") => "opus".to_owned(),
                Some(pcm) if pcm.starts_with("pcm_") => "wav".to_owned(),
                Some(_) | None => {
                    container_extension(probe.container).unwrap_or_else(|| "mka".to_owned())
                }
            }
        }
        AudioTarget::M4a(_)
        | AudioTarget::Mp3(_)
        | AudioTarget::Flac
        | AudioTarget::Wav
        | AudioTarget::Opus(_) => target.extension().slug().to_owned(),
    }
}

fn container_extension(container: Option<MediaContainer>) -> Option<String> {
    container.map(|container| container.extension().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::AudioFacts;
    use anyview_core::{Bitrate, MediaTags, TimeRange};

    fn probe(codec: Option<&str>, container: Option<MediaContainer>) -> MediaProbe {
        MediaProbe {
            length: None,
            container,
            tags: MediaTags::default(),
            video: None,
            audio: codec.map(|codec| AudioFacts {
                codec: codec.to_owned(),
                channels: 2,
                sample_rate: 44_100,
                bitrate: None,
            }),
            tracks: Vec::new(),
            chapters: Vec::new(),
            cover: None,
        }
    }

    fn job(streams: StreamPick, audio: AudioTarget) -> ExportJob {
        ExportJob::Transcode {
            source: FilePath::new("/v/clip.mkv").unwrap(),
            range: TimeRange::WHOLE,
            streams,
            audio,
        }
    }

    #[test]
    fn an_export_is_named_beside_its_source_by_what_it_holds() {
        let mp3 = AudioTarget::Mp3(Bitrate::from_kbps(128));
        let cases: Vec<(&str, &str, ExportJob, MediaProbe, &str)> = vec![
            (
                "trim",
                "/v/clip.mkv",
                job(StreamPick::Everything, AudioTarget::Copy),
                probe(None, None),
                "/v/clip trimmed.mkv",
            ),
            (
                "trim keeps the extension",
                "/v/a.MP4",
                job(StreamPick::Everything, AudioTarget::Copy),
                probe(None, None),
                "/v/a trimmed.MP4",
            ),
            (
                "copied vorbis",
                "/v/clip.mkv",
                job(StreamPick::AudioOnly, AudioTarget::Copy),
                probe(Some("vorbis"), None),
                "/v/clip.ogg",
            ),
            (
                "copied aac",
                "/v/clip.mp4",
                job(StreamPick::AudioOnly, AudioTarget::Copy),
                probe(Some("aac"), None),
                "/v/clip.m4a",
            ),
            (
                "copied flac",
                "/v/tone.flac",
                job(StreamPick::AudioOnly, AudioTarget::Copy),
                probe(Some("flac"), None),
                "/v/tone.flac",
            ),
            (
                "copied pcm",
                "/v/a.avi",
                job(StreamPick::AudioOnly, AudioTarget::Copy),
                probe(Some("pcm_s16le"), None),
                "/v/a.wav",
            ),
            (
                "copied other codec in its container",
                "/v/a.avi",
                job(StreamPick::AudioOnly, AudioTarget::Copy),
                probe(Some("ac3"), Some(MediaContainer::Avi)),
                "/v/a.avi",
            ),
            (
                "converted",
                "/v/tone.flac",
                job(StreamPick::AudioOnly, mp3),
                probe(Some("flac"), None),
                "/v/tone.mp3",
            ),
        ];
        for (name, source, job, probe, want) in cases {
            let source = FilePath::new(source).unwrap();
            let got = output_path(&source, &job, &probe, |_| true).unwrap();
            assert_eq!(got.as_path(), Path::new(want), "{name}");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number_before_its_extension() {
        let source = FilePath::new("/v/tone.flac").unwrap();
        let job = job(StreamPick::AudioOnly, AudioTarget::Flac);
        let probe = probe(Some("flac"), None);
        let taken = ["/v/tone.flac", "/v/tone 2.flac"];
        let free = |path: &Path| !taken.iter().any(|name| Path::new(name) == path);
        let got = output_path(&source, &job, &probe, free).unwrap();
        assert_eq!(got.as_path(), Path::new("/v/tone 3.flac"));
        let nothing_free = output_path(&source, &job, &probe, |_| false);
        assert!(matches!(nothing_free, Err(MediaError::Io { .. })));
    }

    #[test]
    fn only_a_transcode_is_named() {
        let source = FilePath::new("/v/clip.mkv").unwrap();
        let shot = ExportJob::MpvScreenshot {
            target: anyview_core::RasterTarget::Png,
            subtitles: anyview_core::Subtitles::Omit,
        };
        let named = output_path(&source, &shot, &probe(None, None), |_| true);
        assert_eq!(named, Err(MediaError::NotMedia));
    }
}
