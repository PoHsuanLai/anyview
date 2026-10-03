//! The streams of an opened container: the tracks, the facts of the first video and audio, and the
//! cover picture's packet.

use super::{AudioFacts, CoverArt, CoverCodec, VideoFacts};
use anyview_core::{Bitrate, MediaTrack, PixelLen, PixelSize, StreamKind, TrackId, TrackPlay};
use ff::format::stream::Disposition;
use ff::media::Type;
use ffmpeg_next as ff;

/// How many packets are read looking for a cover: the demuxers that attach one hand it over with
/// the first.
const COVER_PACKETS: usize = 16;

/// What the streams say.
pub(super) struct Streams {
    pub(super) tracks: Vec<MediaTrack>,
    pub(super) video: Option<VideoFacts>,
    pub(super) audio: Option<AudioFacts>,
    /// The index of the stream that holds the cover picture.
    pub(super) cover: Option<usize>,
}

/// Read the streams of `input`.
pub(super) fn read(input: &ff::format::context::Input) -> Streams {
    let mut found = Streams {
        tracks: Vec::new(),
        video: None,
        audio: None,
        cover: None,
    };
    let mut counts = [0_u32; 3];
    for stream in input.streams() {
        let parameters = stream.parameters();
        let kind = match parameters.medium() {
            Type::Video => StreamKind::Video,
            Type::Audio => StreamKind::Audio,
            Type::Subtitle => StreamKind::Subtitles,
            Type::Unknown | Type::Data | Type::Attachment => continue,
        };
        let slot = match kind {
            StreamKind::Video => 0,
            StreamKind::Audio => 1,
            StreamKind::Subtitles => 2,
        };
        counts[slot] += 1;
        let codec = parameters.id().name().to_owned();
        let metadata = stream.metadata();
        found.tracks.push(MediaTrack {
            id: TrackId(counts[slot]),
            kind,
            title: metadata.get("title").map(str::to_owned),
            language: metadata.get("language").map(str::to_owned),
            codec: Some(codec.clone()),
            play: TrackPlay::Idle,
        });
        let attached = stream.disposition().contains(Disposition::ATTACHED_PIC);
        match (kind, attached) {
            (StreamKind::Video, true) => {
                found.cover.get_or_insert(stream.index());
            }
            (StreamKind::Video, false) if found.video.is_none() => {
                found.video = video_facts(&stream, codec);
            }
            (StreamKind::Audio, _) if found.audio.is_none() => {
                found.audio = audio_facts(&stream, codec);
            }
            (StreamKind::Video | StreamKind::Audio | StreamKind::Subtitles, _) => {}
        }
    }
    found
}

/// A decoder over the stream's parameters, not opened: libav has copied the size and rate into its
/// context already, and opening a decoder to read them would cost a codec's start-up.
fn unopened(stream: &ff::format::stream::Stream) -> Option<ff::decoder::Opened> {
    let context = ff::codec::Context::from_parameters(stream.parameters()).ok()?;
    Some(ff::decoder::Opened(ff::decoder::Decoder(context)))
}

fn video_facts(stream: &ff::format::stream::Stream, codec: String) -> Option<VideoFacts> {
    let video = ff::decoder::Video(unopened(stream)?);
    Some(VideoFacts {
        codec,
        size: PixelSize {
            width: PixelLen(video.width()),
            height: PixelLen(video.height()),
        },
    })
}

fn audio_facts(stream: &ff::format::stream::Stream, codec: String) -> Option<AudioFacts> {
    let audio = ff::decoder::Audio(unopened(stream)?);
    let bits = u32::try_from(audio.bit_rate()).unwrap_or(0);
    Some(AudioFacts {
        codec,
        channels: audio.channels(),
        sample_rate: audio.rate(),
        bitrate: (bits > 0).then(|| Bitrate::from_kbps(bits / 1000)),
    })
}

/// The cover's packet from the stream at `index`, or `None` when it is missing, larger than
/// `limit` bytes or not a PNG or JPEG.
pub(super) fn cover_of(
    input: &mut ff::format::context::Input,
    index: usize,
    limit: u64,
) -> Option<CoverArt> {
    let id = input.stream(index)?.parameters().id();
    let codec = if id == ff::codec::Id::PNG {
        CoverCodec::Png
    } else if id == ff::codec::Id::MJPEG {
        CoverCodec::Jpeg
    } else {
        return None;
    };
    input
        .packets()
        .take(COVER_PACKETS)
        .find(|(stream, _)| stream.index() == index)
        .and_then(|(_, packet)| packet.data().map(<[u8]>::to_vec))
        .filter(|bytes| u64::try_from(bytes.len()).is_ok_and(|len| len <= limit))
        .map(|bytes| CoverArt { codec, bytes })
}
