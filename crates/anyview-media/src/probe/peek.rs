//! The light tier's peek of video and audio: what `probe` reads, as the rows a pane lists. libav
//! reads headers and a cover only, so the peek stays inside the budget's bytes by passing them as
//! the cover's limit and by never decoding a packet.

use super::{MediaProbe, probe_within};
use crate::MediaError;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatKind, MediaLength, Peek, PeekBudget, Sniffed, Source,
};

/// What a peek holds: the probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPeeked {
    /// What libav read.
    pub probe: MediaProbe,
}

fn peek_of(src: &Source, budget: &PeekBudget) -> Result<MediaPeeked, MediaError> {
    let probe = probe_within(src.path().as_path(), budget.bytes.0)?;
    Ok(MediaPeeked { probe })
}

/// The rows of a probe: how long, how big, how compressed and what the tags say.
fn rows(peeked: &MediaPeeked) -> Facts {
    let probe = &peeked.probe;
    let mut facts = Facts::empty();
    if let Some(length) = probe
        .length
        .filter(|length| *length != MediaLength::default())
    {
        facts = facts.with(FactLabel::Duration, FactValue::duration(length));
    }
    if let Some(video) = &probe.video {
        facts = facts
            .with(FactLabel::Dimensions, FactValue::dimensions(video.size))
            .with(FactLabel::Codec, FactValue::text(video.codec.clone()));
    }
    if let Some(audio) = &probe.audio {
        let label = match probe.video {
            Some(_) => FactLabel::AudioCodec,
            None => FactLabel::Codec,
        };
        facts = facts.with(label, FactValue::text(audio.codec.clone()));
        if let Some(rate) = audio.bitrate {
            facts = facts.with(FactLabel::Bitrate, FactValue::bitrate(rate));
        }
    }
    let tag = |facts: Facts, label, text: &Option<String>| match text {
        Some(text) => facts.with(label, FactValue::text(text.clone())),
        None => facts,
    };
    let facts = tag(facts, FactLabel::Title, &probe.tags.title);
    let facts = tag(facts, FactLabel::Author, &probe.tags.artist);
    tag(facts, FactLabel::Album, &probe.tags.album)
}

/// The peek of a video file.
#[derive(Debug, Clone, Copy)]
pub struct VideoPeek;

impl Peek for VideoPeek {
    const KIND: FormatKind = FormatKind::Video;
    type Peeked = MediaPeeked;
    type Error = MediaError;

    fn peek(src: &Source, _: &Sniffed, budget: &PeekBudget) -> Result<MediaPeeked, MediaError> {
        peek_of(src, budget)
    }

    fn facts(peeked: &MediaPeeked) -> Facts {
        rows(peeked)
    }
}

/// The peek of an audio file.
#[derive(Debug, Clone, Copy)]
pub struct AudioPeek;

impl Peek for AudioPeek {
    const KIND: FormatKind = FormatKind::Audio;
    type Peeked = MediaPeeked;
    type Error = MediaError;

    fn peek(src: &Source, _: &Sniffed, budget: &PeekBudget) -> Result<MediaPeeked, MediaError> {
        peek_of(src, budget)
    }

    fn facts(peeked: &MediaPeeked) -> Facts {
        rows(peeked)
    }
}
