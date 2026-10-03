//! What libav reads of a recording without playing it: its length, tags, tracks, chapters and the
//! cover picture an audio file carries. libav reads the container's header and, for a cover, the
//! first packets, so the cost is the header's and not the recording's; the launcher's pane and the
//! export planner both run on it.

mod container;
mod peek;
mod streams;

use crate::MediaError;
use crate::libav::{micros, start, utf8};
use anyview_core::{
    Bitrate, MediaChapter, MediaContainer, MediaLength, MediaTags, MediaTime, MediaTrack, PixelSize,
};
use ffmpeg_next as ff;
use std::path::Path;

pub use peek::{AudioPeek, MediaPeeked, VideoPeek};

/// The largest cover picture a probe carries when its caller names no limit.
const COVER_LIMIT: u64 = 8 * 1024 * 1024;

/// How an attached picture is encoded: the two formats a cover is stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverCodec {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
}

/// The picture attached to an audio file, still encoded: decoding is the image crate's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArt {
    /// How `bytes` are encoded.
    pub codec: CoverCodec,
    /// The encoded picture.
    pub bytes: Vec<u8>,
}

/// The first picture-carrying video stream of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFacts {
    /// The codec's name, such as `h264`.
    pub codec: String,
    /// The picture's size.
    pub size: PixelSize,
}

/// The first audio stream of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFacts {
    /// The codec's name, such as `flac`.
    pub codec: String,
    /// How many channels it has.
    pub channels: u16,
    /// Samples a second per channel.
    pub sample_rate: u32,
    /// What the stream spends a second, when the container says.
    pub bitrate: Option<Bitrate>,
}

/// A recording as libav describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaProbe {
    /// How long it runs, when the container says.
    pub length: Option<MediaLength>,
    /// The container, when it is one the viewer lists.
    pub container: Option<MediaContainer>,
    /// The title, artist and album tags.
    pub tags: MediaTags,
    /// The video stream, leaving out a cover picture.
    pub video: Option<VideoFacts>,
    /// The audio stream.
    pub audio: Option<AudioFacts>,
    /// Every track, numbered within its kind from 1 as the player numbers them; none is marked
    /// playing, since nothing plays.
    pub tracks: Vec<MediaTrack>,
    /// The chapter marks.
    pub chapters: Vec<MediaChapter>,
    /// The cover picture, for an audio file that has one.
    pub cover: Option<CoverArt>,
}

/// Probe `path`, reading a cover picture up to 8 MiB.
pub fn probe(path: &Path) -> Result<MediaProbe, MediaError> {
    probe_within(path, COVER_LIMIT)
}

/// Probe `path`, leaving out a cover picture larger than `cover_limit` bytes.
pub fn probe_within(path: &Path, cover_limit: u64) -> Result<MediaProbe, MediaError> {
    start()?;
    let mut input = ff::format::input(utf8(path)?).map_err(crate::libav::libav)?;
    let length = length_of(&input);
    let extension = path.extension().and_then(|extension| extension.to_str());
    let container = container::container_of(input.format().name(), extension);
    let found = streams::read(&input);
    let cover = match found.cover {
        Some(index) => streams::cover_of(&mut input, index, cover_limit),
        None => None,
    };
    let source = match found.video {
        Some(_) => TagSource::Container,
        None => TagSource::ContainerOrAudio,
    };
    Ok(MediaProbe {
        length,
        container,
        tags: tags_of(&input, source),
        video: found.video,
        audio: found.audio,
        tracks: found.tracks,
        chapters: chapters_of(&input),
        cover,
    })
}

/// How long the container says it runs: its own duration, else the longest stream's, since a
/// container that cannot say (a raw stream) may still have streams that can.
fn length_of(input: &ff::format::context::Input) -> Option<MediaLength> {
    let whole = input.duration();
    let micros = if whole > 0 {
        whole.unsigned_abs()
    } else {
        input
            .streams()
            .filter(|stream| stream.duration() > 0)
            .map(|stream| micros(stream.duration(), stream.time_base()).0)
            .max()?
    };
    (micros > 0).then_some(MediaLength(MediaTime(micros)))
}

/// Where a recording's own tags may be found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagSource {
    /// The container's.
    Container,
    /// The container's, else those of an audio stream: Ogg keeps an audio file's comments there.
    /// A recording with a picture never takes them, since a stream's title names the track.
    ContainerOrAudio,
}

fn tags_of(input: &ff::format::context::Input, source: TagSource) -> MediaTags {
    let tag = |key: &str| {
        let own = input.metadata().get(key).map(str::to_owned);
        match source {
            TagSource::Container => own,
            TagSource::ContainerOrAudio => own.or_else(|| {
                input
                    .streams()
                    .filter(|stream| stream.parameters().medium() == ff::media::Type::Audio)
                    .find_map(|stream| stream.metadata().get(key).map(str::to_owned))
            }),
        }
    };
    MediaTags {
        title: tag("title"),
        artist: tag("artist"),
        album: tag("album"),
    }
}

fn chapters_of(input: &ff::format::context::Input) -> Vec<MediaChapter> {
    input
        .chapters()
        .map(|chapter| MediaChapter {
            title: chapter
                .metadata()
                .get("title")
                .map(str::to_owned)
                .unwrap_or_default(),
            start: micros(chapter.start(), chapter.time_base()),
        })
        .collect()
}
