//! The peek of video and audio: what the recording's header says and, for a file that carries one,
//! its cover. A recording is never decoded here: the header is read by pure-Rust parsers (see
//! `audio`, `mp4` and `matroska`) and a cover picture is reduced to the pane's size. No library of
//! codecs and no player is in this crate's tree.

use super::cover::AudioCover;
use super::recording::{CoverArt, CoverCodec, Recording};
use super::{audio, matroska, mp4};
use crate::body::Body;
use crate::described::Described;
use crate::error::PeekError;
use crate::frames::cover_picture;
use anyview_core::RasterTarget;
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Input, MediaContainer, Peek,
    PeekBudget, PixelArea, PixelSize, Sniffed,
};
use anyview_image::{ImagePeek, encode};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

/// What a peek of a recording holds: how the file is described, what its header says, and the
/// cover when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLook {
    /// The file's type in words, for the first row of facts and for a recording with no cover.
    pub described: Described,
    /// What the header says, without the cover (that is `cover`, already decoded).
    pub recording: Recording,
    /// How large the file is without its cover, which the bitrate row is worked from.
    pub stream_len: ByteLen,
    /// The cover picture, reduced to the budget.
    pub cover: Option<Arc<ImagePeek>>,
}

/// How a recording's header is read: by which parser, or by none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reader {
    /// symphonia.
    Symphonia,
    /// mp4parse.
    Mp4,
    /// matroska-demuxer.
    Matroska,
    /// Nothing parses it: the facts are the type, size and date.
    Nothing,
}

/// The parser for the container `sniffed` found.
fn reader_for(sniffed: &Sniffed) -> Reader {
    let FormatDetail::Media(container) = sniffed.detail() else {
        return Reader::Nothing;
    };
    match container {
        MediaContainer::Mp3
        | MediaContainer::Aac
        | MediaContainer::M4a
        | MediaContainer::Flac
        | MediaContainer::Wav
        | MediaContainer::Aiff
        | MediaContainer::Ogg
        | MediaContainer::Opus => Reader::Symphonia,
        MediaContainer::Mp4 | MediaContainer::M4v | MediaContainer::Mov => Reader::Mp4,
        MediaContainer::Mkv | MediaContainer::WebM => Reader::Matroska,
        MediaContainer::Avi
        | MediaContainer::MpegTs
        | MediaContainer::Ogv
        | MediaContainer::Mpeg
        | MediaContainer::Wmv
        | MediaContainer::Flv => Reader::Nothing,
    }
}

fn look(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<MediaLook, PeekError> {
    let mut recording = match reader_for(sniffed) {
        Reader::Symphonia => audio::read(src, budget)?,
        Reader::Mp4 => mp4::read(src, budget)?,
        Reader::Matroska => matroska::read(src)?,
        Reader::Nothing => Recording::default(),
    };
    let cover = recording.cover.take();
    // The picture is not the recording's sound or video, so it is left out of the bitrate.
    let cover_len = cover.as_ref().map_or(0, |cover| cover.bytes.len() as u64);
    let cover = cover.and_then(|cover| cover_peek(&cover, budget));
    Ok(MediaLook {
        described: Described::of(sniffed),
        recording,
        stream_len: ByteLen(
            src.stamp()
                .len
                .0
                .max(src.bytes().len().0)
                .saturating_sub(cover_len),
        ),
        cover: cover.map(Arc::new),
    })
}

/// The cover decoded and reduced to the pixels the budget allows; `None` when it will not decode,
/// since a cover is a nicety and the facts still stand.
fn cover_peek(cover: &CoverArt, budget: &PeekBudget) -> Option<ImagePeek> {
    let name = match cover.codec {
        CoverCodec::Png => "cover.png",
        CoverCodec::Jpeg => "cover.jpg",
    };
    cover_picture(&cover.bytes, name, budget)
}

/// The cover of the audio file `src` (a path, or any bytes a host injects) as PNG bytes, reduced
/// to `budget`: ID3 `APIC`, FLAC `PICTURE`, MP4 `covr` or Vorbis `METADATA_BLOCK_PICTURE`. `None`
/// for a file with no picture, one that will not decode, or one that cannot be read.
pub fn audio_cover(
    src: impl Into<Input>,
    sniffed: &Sniffed,
    budget: &PeekBudget,
) -> Option<AudioCover> {
    let src = src.into();
    // A parser fed a hostile header may panic: that is no cover, not a dead worker.
    catch_unwind(AssertUnwindSafe(|| {
        let cover = look(&src, sniffed, budget).ok()?.cover?;
        let png = encode(&cover.picture, RasterTarget::Png).ok()?;
        Some(AudioCover {
            png,
            size: cover.source_size,
        })
    }))
    .ok()
    .flatten()
}

/// The most of a movie's header read for the size of its picture: past it the movie opens at the
/// default window rather than waiting on a header.
const SIZE_HEADER: ByteLen = ByteLen(8 * 1024 * 1024);

/// The size of the first video track's picture, from the header alone: `None` when the container
/// has no reader here, the header is damaged or over [`SIZE_HEADER`], or the file has no video.
pub fn video_size(src: &Input, sniffed: &Sniffed) -> Option<PixelSize> {
    let budget = PeekBudget {
        bytes: SIZE_HEADER,
        pixels: PixelArea(0),
        time: std::time::Duration::ZERO,
    };
    let recording = match reader_for(sniffed) {
        Reader::Mp4 => mp4::read(src, &budget),
        Reader::Matroska => matroska::read(src),
        Reader::Symphonia | Reader::Nothing => return None,
    };
    recording.ok()?.video.map(|video| video.size)
}

/// The kind row, then the rows the header gave.
fn rows(look: &MediaLook) -> Facts {
    look.recording.facts(look.stream_len).rows().iter().fold(
        Facts::empty().with(
            FactLabel::Kind,
            FactValue::text(look.described.kind.clone()),
        ),
        |facts, row| facts.with(row.label, row.value.clone()),
    )
}

/// The peek of a video file: its header's facts, and a picture only when the file carries a cover
/// (the host may add a frame from the thumbnail cache, see `VideoFrames`).
#[derive(Debug, Clone, Copy)]
pub struct VideoPeek;

impl Peek for VideoPeek {
    const KIND: FormatKind = FormatKind::Video;
    type Peeked = MediaLook;
    type Error = PeekError;

    fn peek(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<MediaLook, PeekError> {
        look(src, sniffed, budget)
    }

    fn facts(peeked: &MediaLook) -> Facts {
        rows(peeked)
    }
}

/// The peek of an audio file: facts and the cover it carries.
#[derive(Debug, Clone, Copy)]
pub struct AudioPeek;

impl Peek for AudioPeek {
    const KIND: FormatKind = FormatKind::Audio;
    type Peeked = MediaLook;
    type Error = PeekError;

    fn peek(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<MediaLook, PeekError> {
        look(src, sniffed, budget)
    }

    fn facts(peeked: &MediaLook) -> Facts {
        rows(peeked)
    }
}

impl From<MediaLook> for Body {
    fn from(peeked: MediaLook) -> Self {
        match peeked.cover {
            Some(cover) => Body::Picture(cover),
            None => Body::FactsOnly(peeked.described),
        }
    }
}
