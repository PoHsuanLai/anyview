//! Audio headers through symphonia: MP3, AAC and ALAC in MP4, FLAC, Ogg Vorbis and Opus, WAV and
//! AIFF. Only the container is read, with its tags and attached pictures: no packet is fetched
//! and no decoder is made, so the cost is a header's whatever the length of the recording.

use super::recording::{
    AudioStream, CoverArt, CoverCodec, Recording, TrackCounts, length_of_micros,
};
use crate::error::PeekError;
use anyview_core::{Input, MediaTags, PeekBudget, ReadAtStream};
use std::io::{Read, Seek, SeekFrom};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::well_known as codec;
use symphonia::core::codecs::audio::{AudioCodecId, AudioCodecParameters};
use symphonia::core::common::Limit;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, Track, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{
    MetadataOptions, MetadataRevision, StandardTag, StandardVisualKey, Visual,
};

/// The bytes of an input as symphonia reads them: seekable, of a length known.
struct Stream {
    inner: ReadAtStream,
    len: u64,
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for Stream {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(to)
    }
}

impl MediaSource for Stream {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

/// What a header says of the audio `src` (a path, or any bytes a host injects).
pub fn read(src: &Input, budget: &PeekBudget) -> Result<Recording, PeekError> {
    let file = Stream {
        inner: super::opened(src)?,
        len: src.bytes().len().0,
    };
    let mut hint = Hint::new();
    if let Some(extension) = src.name().extension() {
        hint.with_extension(extension);
    }
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let visual_limit = usize::try_from(budget.bytes.0).unwrap_or(usize::MAX);
    let metadata_options =
        MetadataOptions::default().limit_visual_bytes(Limit::Maximum(visual_limit));
    let mut reader = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), metadata_options)
        .map_err(|error| PeekError::media(error.to_string()))?;
    Ok(recording_of(reader.as_mut(), visual_limit))
}

/// What `reader` knows after probing.
fn recording_of(reader: &mut dyn FormatReader, visual_limit: usize) -> Recording {
    let tracks = reader.tracks();
    let audio_tracks = tracks
        .iter()
        .filter(|track| audio_params(track).is_some())
        .count();
    let first = tracks
        .iter()
        .find_map(|track| audio_params(track).map(|params| (track, params)));
    let audio = first.map(|(_, params)| AudioStream {
        codec: codec_name(params.codec).to_owned(),
        channels: params
            .channels
            .as_ref()
            .and_then(|channels| u16::try_from(channels.count()).ok())
            .filter(|count| *count > 0),
        sample_rate: params.sample_rate,
    });
    let length = media_micros(reader)
        .or_else(|| first.and_then(|(track, params)| track_micros(track, params.sample_rate)));
    let video = reader.first_track(TrackType::Video).is_some();
    let mut recording = Recording {
        length: length.and_then(length_of_micros),
        audio,
        tracks: TrackCounts {
            video: u32::from(video),
            audio: u32::try_from(audio_tracks).unwrap_or(u32::MAX),
            subtitles: 0,
        },
        ..Recording::default()
    };
    let mut metadata = reader.metadata();
    loop {
        if let Some(revision) = metadata.current() {
            take_from(&mut recording, revision, visual_limit);
        }
        if metadata.pop().is_none() {
            break;
        }
    }
    recording
}

/// The audio parameters of `track`. `CodecParameters` is non-exhaustive, so audio is asked for
/// rather than matched against.
fn audio_params(track: &Track) -> Option<&AudioCodecParameters> {
    if let Some(CodecParameters::Audio(params)) = &track.codec_params {
        Some(params)
    } else {
        None
    }
}

/// The whole media's length in microseconds, when the container states one.
fn media_micros(reader: &dyn FormatReader) -> Option<u64> {
    let info = reader.media_info();
    let time = info.time_base?.calc_duration(info.duration?)?;
    u64::try_from(time.as_micros()).ok()
}

/// The track's length in microseconds: its stated duration, else its frames at its sample rate.
fn track_micros(track: &Track, rate: Option<u32>) -> Option<u64> {
    if let (Some(base), Some(duration)) = (track.time_base, track.duration) {
        return base
            .calc_duration(duration)
            .and_then(|time| u64::try_from(time.as_micros()).ok());
    }
    let (frames, rate) = (track.num_frames?, u64::from(rate?));
    (rate > 0).then(|| frames.saturating_mul(1_000_000) / rate)
}

/// Fills what `recording` lacks from one revision of metadata: the first revision to name a tag
/// wins, so a file's own tags come before an ID3v1 footer's.
fn take_from(recording: &mut Recording, revision: &MetadataRevision, visual_limit: usize) {
    let tags = revision.media.tags.iter().chain(
        revision
            .per_track
            .iter()
            .flat_map(|track| track.metadata.tags.iter()),
    );
    for tag in tags {
        let Some(standard) = &tag.std else { continue };
        take_tag(recording, standard);
    }
    if recording.cover.is_none() {
        recording.cover = cover_of(&revision.media.visuals, visual_limit);
    }
}

fn take_tag(recording: &mut Recording, tag: &StandardTag) {
    fill(&mut recording.tags, tag);
    if let StandardTag::TrackNumber(number) = tag
        && recording.track_number.is_none()
    {
        recording.track_number = u32::try_from(*number).ok();
    }
}

/// `tags` with the title, artist or album `tag` names, unless it already has one.
fn fill(tags: &mut MediaTags, tag: &StandardTag) {
    let slot = if let StandardTag::TrackTitle(_) = tag {
        &mut tags.title
    } else if let StandardTag::Artist(_) = tag {
        &mut tags.artist
    } else if let StandardTag::Album(_) = tag {
        &mut tags.album
    } else {
        return;
    };
    let (StandardTag::TrackTitle(text) | StandardTag::Artist(text) | StandardTag::Album(text)) =
        tag
    else {
        return;
    };
    let text = text.trim();
    if slot.is_none() && !text.is_empty() {
        *slot = Some(text.to_owned());
    }
}

/// The front cover, else the first picture, when it is a PNG or JPEG within `limit` bytes.
fn cover_of(visuals: &[Visual], limit: usize) -> Option<CoverArt> {
    let front = visuals
        .iter()
        .find(|visual| visual.usage == Some(StandardVisualKey::FrontCover));
    front
        .into_iter()
        .chain(visuals.iter())
        .filter(|visual| visual.data.len() <= limit)
        .find_map(|visual| {
            CoverCodec::of(&visual.data).map(|codec| CoverArt {
                codec,
                bytes: visual.data.to_vec(),
            })
        })
}

/// The codec's name as a person knows it.
fn codec_name(id: AudioCodecId) -> &'static str {
    const NAMES: &[(AudioCodecId, &str)] = &[
        (codec::CODEC_ID_MP1, "mp1"),
        (codec::CODEC_ID_MP2, "mp2"),
        (codec::CODEC_ID_MP3, "mp3"),
        (codec::CODEC_ID_AAC, "aac"),
        (codec::CODEC_ID_AC3, "ac3"),
        (codec::CODEC_ID_EAC3, "eac3"),
        (codec::CODEC_ID_DCA, "dts"),
        (codec::CODEC_ID_FLAC, "flac"),
        (codec::CODEC_ID_ALAC, "alac"),
        (codec::CODEC_ID_VORBIS, "vorbis"),
        (codec::CODEC_ID_OPUS, "opus"),
        (codec::CODEC_ID_SPEEX, "speex"),
        (codec::CODEC_ID_WAVPACK, "wavpack"),
        (codec::CODEC_ID_WMA, "wma"),
        (codec::CODEC_ID_TTA, "tta"),
        (codec::CODEC_ID_MONKEYS_AUDIO, "ape"),
        (codec::CODEC_ID_TRUEHD, "truehd"),
        (codec::CODEC_ID_PCM_ALAW, "pcm_alaw"),
        (codec::CODEC_ID_PCM_MULAW, "pcm_mulaw"),
    ];
    NAMES
        .iter()
        .find(|(known, _)| *known == id)
        .map_or_else(|| pcm_or_unknown(id), |(_, name)| name)
}

/// The PCM and ADPCM families share one word each (their ids sit together in symphonia's table);
/// anything else is just `audio`.
fn pcm_or_unknown(id: AudioCodecId) -> &'static str {
    if (codec::CODEC_ID_PCM_S32LE..=codec::CODEC_ID_PCM_MULAW).contains(&id) {
        "pcm"
    } else if (codec::CODEC_ID_ADPCM_G722..=codec::CODEC_ID_ADPCM_IMA_QT).contains(&id) {
        "adpcm"
    } else {
        "audio"
    }
}
