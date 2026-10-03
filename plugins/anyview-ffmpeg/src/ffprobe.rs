//! Running `ffprobe` and reading its JSON. Nothing here reads the human-readable output.

use crate::error::FfmpegError;
use crate::units::Micros;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

/// A number ffprobe may print as a JSON number or as a string.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Number {
    /// `"44100"`.
    Text(String),
    /// `2`.
    Int(i64),
    /// `2.5`.
    Float(f64),
}

impl Number {
    /// The value, when it is one.
    pub fn value(&self) -> Option<f64> {
        match self {
            Number::Text(text) => text.trim().parse().ok(),
            Number::Int(n) => Some(*n as f64),
            Number::Float(x) => Some(*x),
        }
    }
}

/// What a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Medium {
    /// Pictures.
    Video,
    /// Sound.
    Audio,
    /// Text over the picture.
    Subtitle,
    /// Anything else ffprobe names.
    #[serde(other)]
    Other,
}

/// The flags of a stream that matter here.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Disposition {
    /// A cover picture rather than a moving one.
    #[serde(default)]
    pub attached_pic: u32,
}

/// One entry of a stream's side data.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SideData {
    /// The display matrix's rotation in degrees.
    pub rotation: Option<Number>,
}

/// One stream of the file.
#[derive(Debug, Clone, Deserialize)]
pub struct Stream {
    /// Its index in the file.
    pub index: u32,
    /// What it carries.
    pub codec_type: Option<Medium>,
    /// The codec's short name.
    pub codec_name: Option<String>,
    /// Width of a picture.
    pub width: Option<u32>,
    /// Height of a picture.
    pub height: Option<u32>,
    /// Samples a second.
    pub sample_rate: Option<Number>,
    /// Channels of sound.
    pub channels: Option<u32>,
    /// The layout's name: `stereo`, `5.1`.
    pub channel_layout: Option<String>,
    /// Bits a second.
    pub bit_rate: Option<Number>,
    /// Pictures a second as a fraction.
    pub avg_frame_rate: Option<String>,
    /// The lowest rate that holds every timestamp, as a fraction.
    pub r_frame_rate: Option<String>,
    /// The shape of a pixel, `1:1`.
    pub sample_aspect_ratio: Option<String>,
    /// How long, in seconds.
    pub duration: Option<Number>,
    /// Flags.
    #[serde(default)]
    pub disposition: Disposition,
    /// The stream's tags.
    #[serde(default)]
    pub tags: HashMap<String, String>,
    /// Side data; the rotation of a phone video is here.
    #[serde(default)]
    pub side_data_list: Vec<SideData>,
}

/// The container.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Format {
    /// How long, in seconds.
    pub duration: Option<Number>,
    /// Bits a second of the whole file.
    pub bit_rate: Option<Number>,
    /// The file's tags.
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

/// What `ffprobe -print_format json -show_format -show_streams -show_chapters` says.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Probed {
    /// The streams in file order.
    #[serde(default)]
    pub streams: Vec<Stream>,
    /// The container.
    #[serde(default)]
    pub format: Format,
    /// The chapters; only their number is used.
    #[serde(default)]
    pub chapters: Vec<serde::de::IgnoredAny>,
}

impl Probed {
    /// Reads ffprobe's JSON.
    pub fn parse(json: &[u8]) -> Result<Probed, FfmpegError> {
        serde_json::from_slice(json).map_err(|error| FfmpegError::Corrupt(error.to_string()))
    }

    /// The first moving picture.
    pub fn video(&self) -> Option<&Stream> {
        self.streams
            .iter()
            .find(|s| s.codec_type == Some(Medium::Video) && s.disposition.attached_pic == 0)
    }

    /// The first cover picture.
    pub fn cover(&self) -> Option<&Stream> {
        self.streams
            .iter()
            .find(|s| s.codec_type == Some(Medium::Video) && s.disposition.attached_pic != 0)
    }

    /// The audio stream FFmpeg would pick without being told: the one with the most channels,
    /// the first of those.
    pub fn audio(&self) -> Option<&Stream> {
        self.streams
            .iter()
            .filter(|s| s.codec_type == Some(Medium::Audio))
            .reduce(|best, s| if s.channels > best.channels { s } else { best })
    }

    /// The stream with this index.
    pub fn stream(&self, index: u32) -> Option<&Stream> {
        self.streams.iter().find(|s| s.index == index)
    }

    /// How long the file runs: the container's say, else the longest stream's.
    pub fn duration(&self) -> Option<Micros> {
        let seconds = |number: &Option<Number>| number.as_ref().and_then(Number::value);
        let whole = seconds(&self.format.duration);
        let longest = self
            .streams
            .iter()
            .filter_map(|s| seconds(&s.duration))
            .fold(None, |best: Option<f64>, d| {
                Some(best.map_or(d, |b| b.max(d)))
            });
        whole.or(longest).and_then(Micros::from_secs_f64)
    }
}

impl Stream {
    /// The picture's turn in degrees, from its display matrix or the old `rotate` tag.
    pub fn rotation(&self) -> i64 {
        let from_side = self
            .side_data_list
            .iter()
            .find_map(|data| data.rotation.as_ref().and_then(Number::value));
        let from_tag = self
            .tags
            .get("rotate")
            .and_then(|tag| tag.parse::<f64>().ok());
        from_side
            .or(from_tag)
            .map_or(0, |degrees| degrees.round() as i64)
    }
}

/// Runs `ffprobe` on `input`.
pub fn probe(ffprobe: &Path, input: &Path) -> Result<Probed, FfmpegError> {
    let output = Command::new(ffprobe)
        .args(["-v", "error", "-print_format", "json"])
        .args(["-show_format", "-show_streams", "-show_chapters", "-i"])
        .arg(input)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| FfmpegError::io("run ffprobe", &error))?;
    if output.status.success() {
        return Probed::parse(&output.stdout);
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let said = said.trim();
    Err(
        if said.contains("No such file") || said.contains("Permission denied") {
            FfmpegError::Unreadable(said.to_owned())
        } else {
            FfmpegError::Corrupt(said.to_owned())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
      "streams": [
        {"index":0,"codec_type":"video","codec_name":"h264","width":64,"height":48,
         "avg_frame_rate":"10/1","disposition":{"attached_pic":0},
         "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]},
        {"index":1,"codec_type":"audio","codec_name":"aac","sample_rate":"48000","channels":2,
         "channel_layout":"stereo","bit_rate":"128000"},
        {"index":2,"codec_type":"audio","codec_name":"ac3","channels":6},
        {"index":3,"codec_type":"video","codec_name":"png","disposition":{"attached_pic":1}},
        {"index":4,"codec_type":"attachment"}
      ],
      "chapters": [{"id":1},{"id":2}],
      "format": {"duration":"3.032000","bit_rate":"500000","tags":{"TITLE":"T"}}
    }"#;

    #[test]
    fn streams_are_picked_the_way_ffmpeg_picks_them() {
        let probed = Probed::parse(JSON.as_bytes()).unwrap();
        assert_eq!(probed.video().map(|s| s.index), Some(0));
        assert_eq!(probed.cover().map(|s| s.index), Some(3));
        assert_eq!(
            probed.audio().map(|s| s.index),
            Some(2),
            "the most channels"
        );
        assert_eq!(probed.chapters.len(), 2);
        assert_eq!(probed.duration(), Some(Micros(3_032_000)));
        assert_eq!(probed.video().map(Stream::rotation), Some(-90));
        assert_eq!(
            probed.stream(4).and_then(|s| s.codec_type),
            Some(Medium::Other)
        );
    }

    #[test]
    fn something_that_is_not_json_is_a_corrupt_file_not_a_panic() {
        assert!(matches!(
            Probed::parse(b"not json"),
            Err(FfmpegError::Corrupt(_))
        ));
        assert!(Probed::parse(b"{}").unwrap().duration().is_none());
    }
}
