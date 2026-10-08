//! The header of an MP4, MOV or M4V through mozilla's `mp4parse`: tracks, sizes, codecs, length,
//! the iTunes tags and the cover. `mp4parse` takes a plain reader and skips what it does not need
//! by reading it, which on a movie's `mdat` is the whole file. So this module finds the `ftyp` and
//! `moov` boxes itself (by seeking over every other box) and gives the parser those alone.

use super::recording::{
    AudioStream, CoverArt, CoverCodec, Recording, TrackCounts, VideoStream, length_of_micros,
};
use crate::error::PeekError;
use anyview_core::{Input, PeekBudget, PixelLen, PixelSize};
use mp4parse::{AudioSampleEntry, CodecType, SampleEntry, Track, TrackType, VideoSampleEntry};
use std::io::{Cursor, Read, Seek, SeekFrom};

/// The most top-level boxes looked at before giving up: a real file has a handful.
const TOP_LEVEL_BOXES: usize = 64;

/// What the header of the movie `src` says.
pub fn read(src: &Input, budget: &PeekBudget) -> Result<Recording, PeekError> {
    let mut file = super::opened(src)?;
    let header =
        movie_header(&mut file, src.bytes().len().0, budget.bytes.0).map_err(PeekError::media)?;
    let context = mp4parse::read_mp4(&mut Cursor::new(&header.bytes))
        .map_err(|error| PeekError::media(format!("not a movie: {error}")))?;
    let mut recording = Recording {
        length: header.micros.and_then(length_of_micros),
        ..Recording::default()
    };
    let mut counts = TrackCounts::default();
    for track in context.tracks.iter() {
        match track.track_type {
            TrackType::Video => {
                counts.video += 1;
                if recording.video.is_none() {
                    recording.video = video_of(track, &header.bytes);
                }
            }
            TrackType::Audio => {
                counts.audio += 1;
                if recording.audio.is_none() {
                    recording.audio = audio_of(track);
                }
            }
            TrackType::Picture
            | TrackType::AuxiliaryVideo
            | TrackType::Metadata
            | TrackType::Unknown => {}
        }
    }
    recording.tracks = counts;
    if let Some(Ok(userdata)) = &context.userdata
        && let Some(meta) = &userdata.meta
    {
        let text = |value: &Option<mp4parse::TryString>| {
            value
                .as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes).trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        recording.tags.title = text(&meta.title);
        recording.tags.artist = text(&meta.artist);
        recording.tags.album = text(&meta.album);
        recording.track_number = meta.track_number.map(u32::from);
        recording.cover = meta.cover_art.as_ref().and_then(|covers| {
            covers.iter().find_map(|cover| {
                CoverCodec::of(cover).map(|codec| CoverArt {
                    codec,
                    bytes: cover.to_vec(),
                })
            })
        });
    }
    Ok(recording)
}

/// The `ftyp` and `moov` boxes of a file, back to back, and the length `moov`'s header states.
struct MovieHeader {
    bytes: Vec<u8>,
    micros: Option<u64>,
}

/// Walks the top-level boxes of `file` (`len` bytes), keeping `ftyp` and `moov` and seeking past
/// the rest. A `moov` larger than `limit` bytes is refused rather than read.
fn movie_header(
    file: &mut (impl Read + Seek),
    len: u64,
    limit: u64,
) -> Result<MovieHeader, String> {
    let mut bytes = Vec::new();
    let mut moov_body = None;
    let mut position = 0_u64;
    for _ in 0..TOP_LEVEL_BOXES {
        if position + 8 > len {
            break;
        }
        file.seek(SeekFrom::Start(position))
            .map_err(|e| e.to_string())?;
        let mut head = [0_u8; 8];
        file.read_exact(&mut head).map_err(|e| e.to_string())?;
        let kind = [head[4], head[5], head[6], head[7]];
        let small = u64::from(u32::from_be_bytes([head[0], head[1], head[2], head[3]]));
        let (size, header_len) = match small {
            0 => (len - position, 8),
            1 => {
                let mut wide = [0_u8; 8];
                file.read_exact(&mut wide).map_err(|e| e.to_string())?;
                (u64::from_be_bytes(wide), 16)
            }
            n => (n, 8),
        };
        if size < header_len || position + size > len {
            return Err("a box runs past the end of the file".to_owned());
        }
        if &kind == b"ftyp" || &kind == b"moov" {
            if size > limit {
                return Err(format!("the movie header is {size} bytes, over the budget"));
            }
            file.seek(SeekFrom::Start(position))
                .map_err(|e| e.to_string())?;
            let mut whole = vec![0_u8; usize::try_from(size).map_err(|e| e.to_string())?];
            file.read_exact(&mut whole).map_err(|e| e.to_string())?;
            if &kind == b"moov" {
                moov_body =
                    Some(bytes.len() + usize::try_from(header_len).map_err(|e| e.to_string())?);
            }
            bytes.extend_from_slice(&whole);
            if moov_body.is_some() {
                break;
            }
        }
        position += size;
    }
    let moov_body = moov_body.ok_or_else(|| "no movie header".to_owned())?;
    let micros = bytes.get(moov_body..).and_then(movie_micros);
    Ok(MovieHeader { bytes, micros })
}

/// The length `mvhd` states, in microseconds: the child of `moov` whose content is `moov`.
fn movie_micros(moov: &[u8]) -> Option<u64> {
    let mut rest = moov;
    while rest.len() >= 8 {
        let size = usize::try_from(u32::from_be_bytes(rest[..4].try_into().ok()?)).ok()?;
        if size < 8 || size > rest.len() {
            return None;
        }
        if &rest[4..8] == b"mvhd" {
            let body = &rest[8..size];
            let (timescale, duration) = match *body.first()? {
                0 => (
                    u64::from(u32::from_be_bytes(body.get(12..16)?.try_into().ok()?)),
                    u64::from(u32::from_be_bytes(body.get(16..20)?.try_into().ok()?)),
                ),
                _ => (
                    u64::from(u32::from_be_bytes(body.get(20..24)?.try_into().ok()?)),
                    u64::from_be_bytes(body.get(24..32)?.try_into().ok()?),
                ),
            };
            return (timescale > 0)
                .then(|| {
                    u64::try_from(u128::from(duration) * 1_000_000 / u128::from(timescale)).ok()
                })
                .flatten();
        }
        rest = &rest[size..];
    }
    None
}

fn video_of(track: &Track, header: &[u8]) -> Option<VideoStream> {
    let entry = track
        .stsd
        .as_ref()?
        .descriptions
        .iter()
        .find_map(|entry| match entry {
            SampleEntry::Video(video) => Some(video),
            SampleEntry::Audio(_) | SampleEntry::Unknown => None,
        });
    let from_header = track
        .tkhd
        .as_ref()
        .map(|tkhd| (tkhd.width >> 16, tkhd.height >> 16));
    let (width, height) = match entry {
        Some(VideoSampleEntry { width, height, .. }) if *width > 0 && *height > 0 => {
            (u32::from(*width), u32::from(*height))
        }
        _ => from_header.filter(|(w, h)| *w > 0 && *h > 0)?,
    };
    let codec = entry
        .and_then(|video| video_codec(video.codec_type))
        .or_else(|| hevc_in(header))
        .unwrap_or("video");
    Some(VideoStream {
        codec: codec.to_owned(),
        size: PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        },
    })
}

fn video_codec(codec: CodecType) -> Option<&'static str> {
    match codec {
        CodecType::H264 => Some("h264"),
        CodecType::MP4V => Some("mpeg4"),
        CodecType::AV1 => Some("av1"),
        CodecType::VP9 => Some("vp9"),
        CodecType::VP8 => Some("vp8"),
        CodecType::H263 => Some("h263"),
        CodecType::Unknown
        | CodecType::MP3
        | CodecType::AAC
        | CodecType::FLAC
        | CodecType::Opus
        | CodecType::EncryptedVideo
        | CodecType::EncryptedAudio
        | CodecType::LPCM
        | CodecType::ALAC => None,
    }
}

/// `hevc` when the header names an HEVC sample entry, which `mp4parse` has no codec type for.
fn hevc_in(header: &[u8]) -> Option<&'static str> {
    const FOURCCS: [&[u8; 4]; 4] = [b"hvc1", b"hev1", b"dvh1", b"dvhe"];
    header
        .windows(4)
        .any(|window| FOURCCS.iter().any(|cc| window == cc.as_slice()))
        .then_some("hevc")
}

fn audio_of(track: &Track) -> Option<AudioStream> {
    let entry: &AudioSampleEntry =
        track
            .stsd
            .as_ref()?
            .descriptions
            .iter()
            .find_map(|entry| match entry {
                SampleEntry::Audio(audio) => Some(audio),
                SampleEntry::Video(_) | SampleEntry::Unknown => None,
            })?;
    let codec = match entry.codec_type {
        CodecType::AAC => "aac",
        CodecType::MP3 => "mp3",
        CodecType::FLAC => "flac",
        CodecType::Opus => "opus",
        CodecType::ALAC => "alac",
        CodecType::LPCM => "pcm",
        CodecType::Unknown
        | CodecType::H264
        | CodecType::MP4V
        | CodecType::AV1
        | CodecType::VP9
        | CodecType::VP8
        | CodecType::EncryptedVideo
        | CodecType::EncryptedAudio
        | CodecType::H263 => "audio",
    };
    Some(AudioStream {
        codec: codec.to_owned(),
        channels: u16::try_from(entry.channelcount)
            .ok()
            .filter(|count| *count > 0),
        sample_rate: (entry.samplerate >= 1.0).then_some(entry.samplerate as u32),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs::File;

    /// A box of `kind` holding `body`.
    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = u32::try_from(body.len() + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn the_movie_header_states_the_length_in_its_own_timescale() {
        // Version 0: created, modified, timescale 600, duration 1800 (3 s).
        let mut v0 = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        v0.extend_from_slice(&600_u32.to_be_bytes());
        v0.extend_from_slice(&1800_u32.to_be_bytes());
        // Version 1: 64-bit times, timescale 1000, duration 2500 (2.5 s).
        let mut v1 = vec![1, 0, 0, 0];
        v1.extend_from_slice(&[0; 16]);
        v1.extend_from_slice(&1000_u32.to_be_bytes());
        v1.extend_from_slice(&2500_u64.to_be_bytes());
        let cases: [(&str, Vec<u8>, Option<u64>); 3] = [
            ("version 0", boxed(b"mvhd", &v0), Some(3_000_000)),
            ("version 1", boxed(b"mvhd", &v1), Some(2_500_000)),
            ("none", boxed(b"free", &[0; 8]), None),
        ];
        for (name, moov, want) in cases {
            assert_eq!(movie_micros(&moov), want, "{name}");
        }
    }

    #[test]
    fn a_truncated_box_is_refused_rather_than_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.mp4");
        let mut bytes = boxed(b"ftyp", b"isom\0\0\0\0");
        bytes.extend_from_slice(&1000_u32.to_be_bytes());
        bytes.extend_from_slice(b"moov");
        std::fs::write(&path, &bytes).unwrap();
        let mut file = File::open(&path).unwrap();
        assert!(movie_header(&mut file, bytes.len() as u64, 1 << 20).is_err());
    }

    #[test]
    fn an_hevc_entry_is_named_though_the_parser_has_no_type_for_it() {
        assert_eq!(hevc_in(b"....stsd....hvc1...."), Some("hevc"));
        assert_eq!(hevc_in(b"....stsd....avc1...."), None);
    }
}
