//! What a recording's header says, as plain values, and the rows a pane lists for it. The readers
//! (`audio`, `mp4`, `matroska`) fill a [`Recording`]; nothing here reads a file.

use anyview_core::{
    Bitrate, ByteLen, FactLabel, FactValue, Facts, MediaLength, MediaTags, MediaTime, PixelSize,
};

/// How an attached picture is encoded: the two formats a cover is stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverCodec {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
}

impl CoverCodec {
    /// The format `bytes` start as, from their magic number.
    pub fn of(bytes: &[u8]) -> Option<CoverCodec> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            Some(CoverCodec::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(CoverCodec::Jpeg)
        } else {
            None
        }
    }
}

/// The picture attached to a recording, still encoded: decoding is the image crate's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArt {
    /// How `bytes` are encoded.
    pub codec: CoverCodec,
    /// The encoded picture.
    pub bytes: Vec<u8>,
}

/// The first picture-carrying video track of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStream {
    /// The codec's name, such as `h264`.
    pub codec: String,
    /// The picture's size.
    pub size: PixelSize,
}

/// The first audio track of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStream {
    /// The codec's name, such as `flac`.
    pub codec: String,
    /// How many channels it has, when the header says.
    pub channels: Option<u16>,
    /// Samples a second per channel, when the header says.
    pub sample_rate: Option<u32>,
}

/// How many tracks of each kind a recording holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrackCounts {
    /// Video tracks, not counting a cover.
    pub video: u32,
    /// Audio tracks.
    pub audio: u32,
    /// Subtitle tracks.
    pub subtitles: u32,
}

/// A recording as its header describes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recording {
    /// How long it runs, when the header says.
    pub length: Option<MediaLength>,
    /// The first video track.
    pub video: Option<VideoStream>,
    /// The first audio track.
    pub audio: Option<AudioStream>,
    /// How many tracks of each kind.
    pub tracks: TrackCounts,
    /// The title, artist and album tags.
    pub tags: MediaTags,
    /// The track's place in its album, from the tag.
    pub track_number: Option<u32>,
    /// The cover picture, when the file carries one.
    pub cover: Option<CoverArt>,
}

impl Recording {
    /// The average bits a second the whole file spends: its size over its length, since a header
    /// seldom states a rate and a peek never reads packets. `None` without a length.
    pub fn bitrate(&self, file: ByteLen) -> Option<Bitrate> {
        let micros = u128::from(self.length?.0.0);
        if micros == 0 {
            return None;
        }
        let kbps = u128::from(file.0) * 8 * 1_000 / micros;
        Some(Bitrate::from_kbps(u32::try_from(kbps).unwrap_or(u32::MAX)))
    }

    /// The rows of a pane: how long, how big, how compressed and what the tags say.
    pub fn facts(&self, file: ByteLen) -> Facts {
        let mut facts = Facts::empty();
        if let Some(length) = self.length.filter(|length| length.0 != MediaTime(0)) {
            facts = facts.with(FactLabel::Duration, FactValue::duration(length));
        }
        if let Some(video) = &self.video {
            facts = facts
                .with(FactLabel::Dimensions, FactValue::dimensions(video.size))
                .with(FactLabel::Codec, FactValue::text(video.codec.clone()));
        }
        if let Some(audio) = &self.audio {
            let label = match self.video {
                Some(_) => FactLabel::AudioCodec,
                None => FactLabel::Codec,
            };
            facts = facts.with(label, FactValue::text(audio.codec.clone()));
            if let Some(rate) = audio.sample_rate {
                facts = facts.with(FactLabel::SampleRate, FactValue::sample_rate(rate));
            }
            if let Some(channels) = audio.channels {
                facts = facts.with(FactLabel::Channels, FactValue::channels(channels));
            }
        }
        if let Some(rate) = self.bitrate(file) {
            facts = facts.with(FactLabel::Bitrate, FactValue::bitrate(rate));
        }
        if let Some(streams) = self.tracks.text() {
            facts = facts.with(FactLabel::Streams, FactValue::text(streams));
        }
        let tag = |facts: Facts, label, text: &Option<String>| match text {
            Some(text) => facts.with(label, FactValue::text(text.clone())),
            None => facts,
        };
        let facts = tag(facts, FactLabel::Title, &self.tags.title);
        let facts = tag(facts, FactLabel::Author, &self.tags.artist);
        let facts = tag(facts, FactLabel::Album, &self.tags.album);
        match self.track_number {
            Some(number) => facts.with(FactLabel::TrackNumber, FactValue::text(number.to_string())),
            None => facts,
        }
    }
}

impl TrackCounts {
    /// `1 video, 2 audio, 3 subtitles`, naming only the kinds present, and only when there is more
    /// than the one stream a plain file has: a single track says nothing the codec row does not.
    fn text(&self) -> Option<String> {
        let total = self.video + self.audio + self.subtitles;
        if total < 2 {
            return None;
        }
        let parts: Vec<String> = [
            (self.video, "video"),
            (self.audio, "audio"),
            (self.subtitles, "subtitles"),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, name)| format!("{count} {name}"))
        .collect();
        Some(parts.join(", "))
    }
}

/// `micros` microseconds as a length, or `None` when it is zero.
pub fn length_of_micros(micros: u64) -> Option<MediaLength> {
    (micros > 0).then_some(MediaLength(MediaTime(micros)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::PixelLen;
    use ds_core::word::Word;

    fn seconds(secs: u64) -> Option<MediaLength> {
        length_of_micros(secs * 1_000_000)
    }

    #[test]
    fn a_cover_is_told_by_its_magic_number() {
        const CASES: &[(&str, &[u8], Option<CoverCodec>)] = &[
            ("png", &[0x89, b'P', b'N', b'G', 0], Some(CoverCodec::Png)),
            ("jpeg", &[0xFF, 0xD8, 0xFF, 0xE0], Some(CoverCodec::Jpeg)),
            ("webp", b"RIFF....WEBP", None),
            ("empty", &[], None),
        ];
        for (name, bytes, want) in CASES {
            assert_eq!(CoverCodec::of(bytes), *want, "{name}");
        }
    }

    #[test]
    fn the_bitrate_is_the_file_over_its_length() {
        let recording = Recording {
            length: seconds(10),
            ..Recording::default()
        };
        assert_eq!(
            recording.bitrate(ByteLen(160_000)).map(|rate| rate.kbps()),
            Some(128)
        );
        assert_eq!(Recording::default().bitrate(ByteLen(1)), None);
    }

    #[test]
    fn the_rows_follow_the_order_a_pane_lists_them() {
        let recording = Recording {
            length: seconds(3),
            video: Some(VideoStream {
                codec: "h264".into(),
                size: PixelSize {
                    width: PixelLen(64),
                    height: PixelLen(48),
                },
            }),
            audio: Some(AudioStream {
                codec: "aac".into(),
                channels: Some(2),
                sample_rate: Some(44_100),
            }),
            tracks: TrackCounts {
                video: 1,
                audio: 1,
                subtitles: 0,
            },
            tags: MediaTags {
                title: Some("T".into()),
                artist: None,
                album: Some("A".into()),
            },
            track_number: Some(4),
            cover: None,
        };
        let slugs: Vec<&str> = recording
            .facts(ByteLen(30_000))
            .rows()
            .iter()
            .map(|row| Word::slug(row.label))
            .collect();
        assert_eq!(
            slugs,
            [
                "duration",
                "dimensions",
                "codec",
                "audio-codec",
                "sample-rate",
                "channels",
                "bitrate",
                "streams",
                "title",
                "album",
                "track-number"
            ]
        );
    }

    #[test]
    fn a_single_track_lists_no_streams_row() {
        const CASES: &[(&str, TrackCounts, Option<&str>)] = &[
            (
                "one",
                TrackCounts {
                    video: 0,
                    audio: 1,
                    subtitles: 0,
                },
                None,
            ),
            (
                "several",
                TrackCounts {
                    video: 1,
                    audio: 2,
                    subtitles: 3,
                },
                Some("1 video, 2 audio, 3 subtitles"),
            ),
        ];
        for (name, counts, want) in CASES {
            assert_eq!(counts.text().as_deref(), *want, "{name}");
        }
    }
}
