//! Which audio encoders this system's libav has. A distribution builds libav with the codecs it may
//! ship, so what a conversion can write is found out, not assumed, and a missing one is reported by
//! name instead of failing half-way through a file.

use crate::MediaError;
use crate::libav::start;
use anyview_core::AudioTarget;
use ds_core::word::Word;
use ffmpeg_next as ff;

/// A format audio is converted to: an [`AudioTarget`] without its options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum AudioFormat {
    /// AAC in an M4A file.
    M4a,
    /// MP3.
    Mp3,
    /// FLAC.
    Flac,
    /// 16-bit WAV.
    Wav,
    /// Opus in an Ogg file.
    Opus,
}

impl AudioFormat {
    /// The format `target` writes, or `None` for a copy, which needs no encoder.
    pub fn of(target: AudioTarget) -> Option<AudioFormat> {
        match target {
            AudioTarget::Copy => None,
            AudioTarget::M4a(_) => Some(AudioFormat::M4a),
            AudioTarget::Mp3(_) => Some(AudioFormat::Mp3),
            AudioTarget::Flac => Some(AudioFormat::Flac),
            AudioTarget::Wav => Some(AudioFormat::Wav),
            AudioTarget::Opus(_) => Some(AudioFormat::Opus),
        }
    }

    /// The encoders that write the format, best first: the native AAC and FLAC and PCM, LAME for
    /// MP3, and libopus before libav's experimental native Opus.
    fn candidates(self) -> &'static [&'static str] {
        match self {
            AudioFormat::M4a => &["aac"],
            AudioFormat::Mp3 => &["libmp3lame"],
            AudioFormat::Flac => &["flac"],
            AudioFormat::Wav => &["pcm_s16le"],
            AudioFormat::Opus => &["libopus", "opus"],
        }
    }
}

/// Whether the system can encode a format, and with which encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    /// This libav encoder writes it.
    Available(&'static str),
    /// No encoder for it.
    Missing,
}

/// What the system's libav can write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoders {
    found: Vec<(AudioFormat, Encoder)>,
}

impl Encoders {
    /// Ask libav for each format's encoders.
    pub fn detect() -> Result<Encoders, MediaError> {
        start()?;
        Ok(Encoders::detect_with(|name| {
            ff::encoder::find_by_name(name).is_some()
        }))
    }

    /// The report for a system where `has` says which encoder names exist: how a test makes a
    /// system with one missing.
    pub fn detect_with(has: impl Fn(&str) -> bool) -> Encoders {
        let found = AudioFormat::ALL
            .iter()
            .map(|format| {
                let encoder = format
                    .candidates()
                    .iter()
                    .find(|name| has(name))
                    .map_or(Encoder::Missing, |name| Encoder::Available(name));
                (*format, encoder)
            })
            .collect();
        Encoders { found }
    }

    /// The encoder for `format`.
    pub fn encoder(&self, format: AudioFormat) -> Encoder {
        self.found
            .iter()
            .find(|(candidate, _)| *candidate == format)
            .map_or(Encoder::Missing, |(_, encoder)| *encoder)
    }

    /// The formats with no encoder.
    pub fn missing(&self) -> Vec<AudioFormat> {
        self.found
            .iter()
            .filter(|(_, encoder)| *encoder == Encoder::Missing)
            .map(|(format, _)| *format)
            .collect()
    }

    /// The name of the encoder that writes `target`: `None` for a copy, which needs none, and an
    /// error naming the format when it has none here.
    pub fn choose(&self, target: AudioTarget) -> Result<Option<&'static str>, MediaError> {
        let Some(format) = AudioFormat::of(target) else {
            return Ok(None);
        };
        match self.encoder(format) {
            Encoder::Available(name) => Ok(Some(name)),
            Encoder::Missing => Err(MediaError::EncoderMissing(format.label())),
        }
    }

    /// Whether `target` can be written here.
    pub fn supports(&self, target: AudioTarget) -> Result<(), MediaError> {
        self.choose(target).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::Bitrate;

    #[test]
    fn a_missing_encoder_is_named_and_a_copy_never_needs_one() {
        let no_opus = Encoders::detect_with(|name| !name.contains("opus"));
        assert_eq!(no_opus.missing(), vec![AudioFormat::Opus]);
        let opus = AudioTarget::Opus(Bitrate::from_kbps(96));
        assert_eq!(
            no_opus.supports(opus),
            Err(MediaError::EncoderMissing("Opus"))
        );
        assert_eq!(no_opus.choose(AudioTarget::Flac), Ok(Some("flac")));
        let none = Encoders::detect_with(|_| false);
        assert_eq!(none.missing(), AudioFormat::ALL.to_vec());
        assert_eq!(none.choose(AudioTarget::Copy), Ok(None));
    }

    #[test]
    fn libopus_is_preferred_to_the_experimental_native_encoder() {
        let both = Encoders::detect_with(|_| true);
        assert_eq!(
            both.encoder(AudioFormat::Opus),
            Encoder::Available("libopus")
        );
        let native = Encoders::detect_with(|name| name == "opus");
        assert_eq!(
            native.encoder(AudioFormat::Opus),
            Encoder::Available("opus")
        );
    }
}
