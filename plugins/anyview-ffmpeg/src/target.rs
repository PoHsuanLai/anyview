//! The export targets, as the manifest spells them.

/// What an export writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// The recording cut to a range by stream copy, in the container the output's name asks for.
    Trim,
    /// The audio track as it is, with no re-encode.
    AudioCopy,
    /// AAC.
    M4a,
    /// MP3.
    Mp3,
    /// Lossless FLAC.
    Flac,
    /// 16-bit PCM.
    Wav,
    /// Opus.
    Opus,
}

impl Target {
    /// Every target, in the order the manifest lists them.
    pub const ALL: [Target; 7] = [
        Target::Trim,
        Target::AudioCopy,
        Target::M4a,
        Target::Mp3,
        Target::Flac,
        Target::Wav,
        Target::Opus,
    ];

    /// The name the manifest and the protocol use.
    pub fn slug(self) -> &'static str {
        match self {
            Target::Trim => "trim",
            Target::AudioCopy => "audio-copy",
            Target::M4a => "m4a",
            Target::Mp3 => "mp3",
            Target::Flac => "flac",
            Target::Wav => "wav",
            Target::Opus => "opus",
        }
    }

    /// The target a name spells.
    pub fn parse(slug: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|target| target.slug() == slug)
    }

    /// The encoders that write it, best first; none for a copy.
    pub fn encoders(self) -> &'static [&'static str] {
        match self {
            Target::Trim | Target::AudioCopy => &[],
            Target::M4a => &["aac"],
            Target::Mp3 => &["libmp3lame"],
            Target::Flac => &["flac"],
            Target::Wav => &["pcm_s16le"],
            Target::Opus => &["libopus", "opus"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slug_parses_back_and_fits_the_manifest_alphabet() {
        for target in Target::ALL {
            assert_eq!(Target::parse(target.slug()), Some(target));
            assert!(
                target
                    .slug()
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c))
            );
        }
        assert_eq!(Target::parse("ogg"), None);
    }
}
