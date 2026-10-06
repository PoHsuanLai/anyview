//! One recording opened for decoding: symphonia reads the container and decodes the first audio
//! track, and what comes out is interleaved 32-bit float sound in the track's own format.

use super::format::StreamFormat;
use crate::error::MediaError;
use anyview_core::{MediaLength, MediaTime};
use std::fs::File;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Duration, TimeBase, Timestamp};

/// How many packets in a row may fail to decode before the recording is given up on: a damaged
/// frame is skipped, a stream of them is not a recording.
const MAX_BAD_PACKETS: u32 = 64;

/// The format of a source whose first buffer has not been decoded yet.
const UNKNOWN: StreamFormat = StreamFormat {
    rate: NonZeroU32::MIN,
    channels: NonZeroU16::MIN,
};

/// What a read of the recording found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Read {
    /// Some frames were appended.
    Frames,
    /// The recording ends here.
    End,
}

/// A recording opened for decoding.
pub(super) struct Source {
    path: PathBuf,
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    base: Option<TimeBase>,
    format: StreamFormat,
    length: Option<MediaLength>,
    codec: String,
    /// Decoded frames still to be thrown away: an accurate seek lands before its target.
    discard: u64,
    /// The first buffer, decoded to learn the format and handed over by the first read.
    primed: Vec<f32>,
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source")
            .field("path", &self.path)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

/// The error of a symphonia call, as this crate reports it.
fn fault(error: &SymphoniaError) -> MediaError {
    match error {
        SymphoniaError::Unsupported(what) => MediaError::NoDecoder((*what).to_owned()),
        other => MediaError::Decode(other.to_string()),
    }
}

impl Source {
    /// Open `path`: probe its container, make a decoder for its first audio track and decode the
    /// first buffer. A recording with a picture track is a video, which this player does not play.
    pub(super) fn open(path: &Path) -> Result<Source, MediaError> {
        let file = File::open(path).map_err(|error| MediaError::Io {
            op: "read",
            path: path.to_path_buf(),
            kind: error.kind(),
        })?;
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            hint.with_extension(extension);
        }
        let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let reader = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|error| fault(&error))?;
        if reader.first_track(TrackType::Video).is_some() {
            return Err(MediaError::NoDecoder("a picture".to_owned()));
        }
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| MediaError::Decode("the file has no audio".to_owned()))?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| MediaError::Decode("the audio track has no parameters".to_owned()))?;
        if is_beyond_aac_lc(params) {
            return Err(MediaError::NoDecoder("HE-AAC".to_owned()));
        }
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|error| match error {
                SymphoniaError::Unsupported(_) => {
                    MediaError::NoDecoder(codec_name(path, params.codec))
                }
                other => MediaError::Decode(other.to_string()),
            })?;
        let (id, base) = (track.id, track.time_base);
        let codec = codec_name(path, params.codec);
        let length = length_of(track, params.sample_rate, reader.as_ref());
        let mut source = Source {
            path: path.to_path_buf(),
            reader,
            decoder,
            track: id,
            base,
            format: UNKNOWN,
            length,
            codec,
            discard: 0,
            primed: Vec::new(),
        };
        source.prime()?;
        Ok(source)
    }

    /// Decode the first buffer, which says what format the recording is in.
    fn prime(&mut self) -> Result<(), MediaError> {
        let mut first = Vec::new();
        let Some(format) = self.next_buffer(None, &mut first)? else {
            return Err(MediaError::Decode("the recording has no sound".to_owned()));
        };
        self.format = format;
        self.primed = first;
        Ok(())
    }

    /// The format of the sound this source decodes to.
    pub(super) fn format(&self) -> StreamFormat {
        self.format
    }

    /// How long the recording runs, when its container says.
    pub(super) fn length(&self) -> Option<MediaLength> {
        self.length
    }

    /// The codec's name, as the track list shows it.
    pub(super) fn codec(&self) -> &str {
        &self.codec
    }

    /// Append the next stretch of sound to `out` in this source's format.
    pub(super) fn read(&mut self, out: &mut Vec<f32>) -> Result<Read, MediaError> {
        if !self.primed.is_empty() {
            let mut primed = std::mem::take(&mut self.primed);
            let width = self.format.width();
            let thrown = self.discard.min((primed.len() / width) as u64);
            self.discard -= thrown;
            primed.drain(..thrown as usize * width);
            if !primed.is_empty() {
                out.append(&mut primed);
                return Ok(Read::Frames);
            }
        }
        match self.next_buffer(Some(self.format), out)? {
            Some(_) => Ok(Read::Frames),
            None => Ok(Read::End),
        }
    }

    /// Decode packets until one yields frames to keep, appended to `out`, and say its format; or
    /// `None` at the end of the recording. A format other than `expected` is an error.
    fn next_buffer(
        &mut self,
        expected: Option<StreamFormat>,
        out: &mut Vec<f32>,
    ) -> Result<Option<StreamFormat>, MediaError> {
        let mut bad = 0;
        let mut samples = Vec::new();
        loop {
            let packet = match self.reader.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                // A file cut short ends where it was cut.
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(fault(&error)),
            };
            if packet.track_id != self.track {
                continue;
            }
            let (trim_start, trim_end) = (packet.trim_start, packet.trim_end);
            let buffer = match self.decoder.decode(&packet) {
                Ok(buffer) => buffer,
                Err(SymphoniaError::IoError(_) | SymphoniaError::DecodeError(_)) => {
                    bad += 1;
                    if bad > MAX_BAD_PACKETS {
                        return Err(MediaError::Decode("the stream is damaged".to_owned()));
                    }
                    continue;
                }
                Err(error) => return Err(fault(&error)),
            };
            bad = 0;
            let spec = buffer.spec();
            let format = u16::try_from(spec.channels().count())
                .ok()
                .and_then(|channels| StreamFormat::new(spec.rate(), channels))
                .ok_or_else(|| MediaError::Decode("the stream has no format".to_owned()))?;
            if expected.is_some_and(|expected| expected != format) {
                return Err(MediaError::Decode("the stream changes format".to_owned()));
            }
            buffer.copy_to_vec_interleaved::<f32>(&mut samples);
            let width = format.width();
            let cut_front = self.frames_of(trim_start, format) as usize * width;
            let cut_back = self.frames_of(trim_end, format) as usize * width;
            let end = samples.len().saturating_sub(cut_back);
            let kept = &samples[cut_front.min(end)..end];
            let thrown = self.discard.min((kept.len() / width) as u64);
            self.discard -= thrown;
            let kept = &kept[thrown as usize * width..];
            if kept.is_empty() {
                continue;
            }
            out.extend_from_slice(kept);
            return Ok(Some(format));
        }
    }

    /// The frames `duration` (in the track's own time units) lasts at `format`'s rate.
    fn frames_of(&self, duration: Duration, format: StreamFormat) -> u64 {
        match self.base {
            Some(base) => {
                let nanos = base.calc_duration_saturating(duration).as_nanos().max(0);
                u64::try_from(nanos * i128::from(format.rate.get()) / 1_000_000_000)
                    .unwrap_or(u64::MAX)
            }
            None => duration.get(),
        }
    }

    /// The frame a timestamp of the track falls on.
    fn frame_at(&self, ts: Timestamp) -> u64 {
        match self.base {
            Some(base) => {
                let nanos = base.calc_time_saturating(ts).as_nanos().max(0);
                u64::try_from(nanos * i128::from(self.format.rate.get()) / 1_000_000_000)
                    .unwrap_or(u64::MAX)
            }
            None => u64::try_from(ts.get()).unwrap_or(0),
        }
    }

    /// Move to `to`, so the next read starts at it. A container that cannot seek there is read
    /// again from its start and the sound before `to` thrown away: slow for a long file, exact.
    pub(super) fn seek(&mut self, to: MediaTime) -> Result<(), MediaError> {
        self.primed.clear();
        let nanos = i64::try_from(to.0.saturating_mul(1_000)).unwrap_or(i64::MAX);
        let target = symphonia::core::units::Time::from_nanos(nanos);
        let sought = self.reader.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: target,
                track_id: Some(self.track),
            },
        );
        match sought {
            Ok(landed) => {
                self.decoder.reset();
                self.discard = self
                    .frame_at(landed.required_ts)
                    .saturating_sub(self.frame_at(landed.actual_ts));
                Ok(())
            }
            Err(SymphoniaError::SeekError(_)) => self.replay_to(to),
            Err(error) => Err(fault(&error)),
        }
    }

    fn replay_to(&mut self, to: MediaTime) -> Result<(), MediaError> {
        let again = Source::open(&self.path)?;
        self.reader = again.reader;
        self.decoder = again.decoder;
        self.primed = again.primed;
        self.discard = self.format.frames_in(to.0);
        Ok(())
    }
}

/// Whether the track is AAC of a profile the decoder does not have: it decodes the low-complexity
/// profile, and the spectral band replication of HE-AAC would play at the wrong rate.
fn is_beyond_aac_lc(params: &symphonia::core::codecs::audio::AudioCodecParameters) -> bool {
    use symphonia::core::codecs::audio::well_known::profiles::{
        CODEC_PROFILE_AAC_HE, CODEC_PROFILE_AAC_HE_V2, CODEC_PROFILE_AAC_USAC,
    };
    [
        CODEC_PROFILE_AAC_HE,
        CODEC_PROFILE_AAC_HE_V2,
        CODEC_PROFILE_AAC_USAC,
    ]
    .into_iter()
    .any(|profile| params.profile == Some(profile))
}

/// The codec's name for the track list: the extension's family when symphonia has no name.
fn codec_name(path: &Path, id: symphonia::core::codecs::audio::AudioCodecId) -> String {
    use symphonia::core::codecs::audio::well_known as codec;
    let name = match id {
        codec::CODEC_ID_MP3 => "mp3",
        codec::CODEC_ID_AAC => "aac",
        codec::CODEC_ID_ALAC => "alac",
        codec::CODEC_ID_FLAC => "flac",
        codec::CODEC_ID_VORBIS => "vorbis",
        codec::CODEC_ID_OPUS => "opus",
        _ => {
            return path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("audio")
                .to_ascii_lowercase();
        }
    };
    name.to_owned()
}

/// The recording's length: the track's own count of frames, else its stated duration, else the
/// container's.
fn length_of(
    track: &symphonia::core::formats::Track,
    rate: Option<u32>,
    reader: &dyn FormatReader,
) -> Option<MediaLength> {
    let micros = match (track.num_frames, rate) {
        (Some(frames), Some(rate)) if rate > 0 => {
            u64::try_from(u128::from(frames) * 1_000_000 / u128::from(rate)).ok()
        }
        _ => None,
    }
    .or_else(|| {
        let (base, duration) = (track.time_base?, track.duration?);
        u64::try_from(base.calc_duration(duration)?.as_micros()).ok()
    })
    .or_else(|| {
        let info = reader.media_info();
        u64::try_from(info.time_base?.calc_duration(info.duration?)?.as_micros()).ok()
    })?;
    (micros > 0).then_some(MediaLength(MediaTime(micros)))
}
