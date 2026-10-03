//! Which audio encoders this FFmpeg has, found by asking it once.

use crate::error::FfmpegError;
use crate::target::Target;
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};

/// The audio encoders `ffmpeg -encoders` lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Encoders(HashSet<String>);

impl Encoders {
    /// Asks the program at `ffmpeg`.
    pub fn detect(ffmpeg: &Path) -> Result<Encoders, FfmpegError> {
        let output = Command::new(ffmpeg)
            .args(["-hide_banner", "-encoders"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|error| FfmpegError::io("list the encoders", &error))?;
        Ok(Encoders::parse(&String::from_utf8_lossy(&output.stdout)))
    }

    /// The audio encoders in the text of `-encoders`: rows of flags (`A....D` for audio), a name
    /// and a description, after a line of dashes.
    pub fn parse(text: &str) -> Encoders {
        let names = text
            .lines()
            .skip_while(|line| !line.trim_start().starts_with("---"))
            .skip(1)
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let flags = words.next()?;
                let name = words.next()?;
                flags.starts_with('A').then(|| name.to_owned())
            });
        Encoders(names.collect())
    }

    /// The encoder that writes `target`: none for a copy, which needs one, and none when the
    /// system has none.
    pub fn for_target(&self, target: Target) -> Option<&'static str> {
        target
            .encoders()
            .iter()
            .copied()
            .find(|name| self.0.contains(*name))
    }

    /// Whether `target` can be written here: a copy always, an encoding when an encoder is there.
    pub fn can_write(&self, target: Target) -> bool {
        target.encoders().is_empty() || self.for_target(target).is_some()
    }

    /// The targets that can be written here, in manifest order.
    pub fn targets(&self) -> Vec<Target> {
        Target::ALL
            .into_iter()
            .filter(|target| self.can_write(*target))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "Encoders:
 V..... = Video
 A..... = Audio
 ------
 V....D libx265              libx265 H.265
 A....D aac                  AAC (Advanced Audio Coding)
 A....D flac                 FLAC (Free Lossless Audio Codec)
 A..X.D opus                 Opus
 A....D pcm_s16le            PCM signed 16-bit little-endian
";

    #[test]
    fn only_audio_encoders_are_read_and_a_missing_one_drops_its_target() {
        let encoders = Encoders::parse(LISTING);
        assert_eq!(encoders.for_target(Target::M4a), Some("aac"));
        assert_eq!(encoders.for_target(Target::Mp3), None);
        assert_eq!(encoders.for_target(Target::Opus), Some("opus"));
        let slugs: Vec<_> = encoders.targets().into_iter().map(Target::slug).collect();
        assert_eq!(slugs, ["trim", "audio-copy", "m4a", "flac", "wav", "opus"]);
    }

    #[test]
    fn libopus_is_preferred_to_the_native_encoder() {
        let both = Encoders::parse(&format!("{LISTING} A....D libopus              libopus\n"));
        assert_eq!(both.for_target(Target::Opus), Some("libopus"));
    }

    #[test]
    fn nothing_listed_leaves_only_the_copies() {
        let none = Encoders::default();
        let slugs: Vec<_> = none.targets().into_iter().map(Target::slug).collect();
        assert_eq!(slugs, ["trim", "audio-copy"]);
    }
}
