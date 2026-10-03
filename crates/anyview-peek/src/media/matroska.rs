//! The header of a Matroska or WebM file through `matroska-demuxer`: the segment's length, its
//! tracks (sizes and codec ids) and its tags. The demuxer reads up to the first cluster and no
//! further, and it has no accessor for attachments, so a cover picture is not read here.

use super::recording::{AudioStream, Recording, TrackCounts, VideoStream, length_of_micros};
use crate::error::PeekError;
use anyview_core::{PixelLen, PixelSize, Source};
use matroska_demuxer::{MatroskaFile, TrackEntry, TrackType};
use std::fs::File;
use std::io::BufReader;

/// What the header of the Matroska file at `src` says.
pub fn read(src: &Source) -> Result<Recording, PeekError> {
    let path = src.path().as_path();
    let file = File::open(path).map_err(|error| PeekError::Unreadable {
        path: path.to_path_buf(),
        kind: error.kind(),
    })?;
    let matroska = MatroskaFile::open(BufReader::new(file))
        .map_err(|error| PeekError::media(error.to_string()))?;
    let info = matroska.info();
    let micros = info.duration().map(|ticks| {
        // The duration is in units of the timestamp scale, which is nanoseconds a tick by default.
        let nanos = ticks * info.timestamp_scale().get() as f64;
        (nanos / 1_000.0).max(0.0) as u64
    });
    let mut recording = Recording {
        length: micros.and_then(length_of_micros),
        ..Recording::default()
    };
    recording.tags.title = info
        .title()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned);
    let mut counts = TrackCounts::default();
    for track in matroska.tracks() {
        match track.track_type() {
            TrackType::Video => {
                counts.video += 1;
                if recording.video.is_none() {
                    recording.video = video_of(track);
                }
            }
            TrackType::Audio => {
                counts.audio += 1;
                if recording.audio.is_none() {
                    recording.audio = audio_of(track);
                }
            }
            TrackType::Subtitle => counts.subtitles += 1,
            TrackType::Unknown
            | TrackType::Complex
            | TrackType::Logo
            | TrackType::Buttons
            | TrackType::Control
            | TrackType::Metadata => {}
        }
    }
    recording.tracks = counts;
    for tag in matroska.tags().unwrap_or_default() {
        // A tag aimed at one track says nothing of the recording.
        if tag
            .targets()
            .and_then(|targets| targets.tag_track_uid())
            .is_some()
        {
            continue;
        }
        for simple in tag.simple_tags() {
            let Some(value) = simple
                .string()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let slot = match simple.name().to_ascii_uppercase().as_str() {
                "TITLE" => &mut recording.tags.title,
                "ARTIST" => &mut recording.tags.artist,
                "ALBUM" => &mut recording.tags.album,
                _ => continue,
            };
            if slot.is_none() || simple.name().eq_ignore_ascii_case("TITLE") {
                *slot = Some(value.to_owned());
            }
        }
    }
    Ok(recording)
}

fn video_of(track: &TrackEntry) -> Option<VideoStream> {
    let video = track.video()?;
    Some(VideoStream {
        codec: codec_name(track.codec_id()),
        size: PixelSize {
            width: PixelLen(u32::try_from(video.pixel_width().get()).ok()?),
            height: PixelLen(u32::try_from(video.pixel_height().get()).ok()?),
        },
    })
}

fn audio_of(track: &TrackEntry) -> Option<AudioStream> {
    let audio = track.audio();
    Some(AudioStream {
        codec: codec_name(track.codec_id()),
        channels: audio.and_then(|audio| u16::try_from(audio.channels().get()).ok()),
        sample_rate: audio
            .map(|audio| audio.sampling_frequency() as u32)
            .filter(|rate| *rate > 0),
    })
}

/// A Matroska codec id as the name a person knows: `V_MPEG4/ISO/AVC` is `h264`.
fn codec_name(id: &str) -> String {
    const NAMES: &[(&str, &str)] = &[
        ("V_MPEG4/ISO/AVC", "h264"),
        ("V_MPEGH/ISO/HEVC", "hevc"),
        ("V_MPEG4/ISO/ASP", "mpeg4"),
        ("V_MPEG4/ISO/SP", "mpeg4"),
        ("V_MPEG4/ISO/AP", "mpeg4"),
        ("V_MPEG4/MS/V3", "msmpeg4v3"),
        ("V_MPEG2", "mpeg2video"),
        ("V_MPEG1", "mpeg1video"),
        ("V_VP8", "vp8"),
        ("V_VP9", "vp9"),
        ("V_AV1", "av1"),
        ("V_THEORA", "theora"),
        ("A_VORBIS", "vorbis"),
        ("A_OPUS", "opus"),
        ("A_FLAC", "flac"),
        ("A_AC3", "ac3"),
        ("A_EAC3", "eac3"),
        ("A_DTS", "dts"),
        ("A_TRUEHD", "truehd"),
        ("A_MPEG/L3", "mp3"),
        ("A_MPEG/L2", "mp2"),
        ("A_ALAC", "alac"),
    ];
    if let Some((_, name)) = NAMES.iter().find(|(known, _)| *known == id) {
        return (*name).to_owned();
    }
    if id.starts_with("A_AAC") {
        "aac".to_owned()
    } else if id.starts_with("A_PCM") {
        "pcm".to_owned()
    } else {
        id.split_once('_')
            .map_or(id, |(_, rest)| rest)
            .to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_codec_id_is_the_name_a_person_knows() {
        const CASES: &[(&str, &str)] = &[
            ("V_MPEG4/ISO/AVC", "h264"),
            ("V_MPEGH/ISO/HEVC", "hevc"),
            ("A_AAC/MPEG4/LC", "aac"),
            ("A_PCM/INT/LIT", "pcm"),
            ("A_VORBIS", "vorbis"),
            ("S_TEXT/UTF8", "text/utf8"),
        ];
        for (id, want) in CASES {
            assert_eq!(codec_name(id), *want, "{id}");
        }
    }
}
