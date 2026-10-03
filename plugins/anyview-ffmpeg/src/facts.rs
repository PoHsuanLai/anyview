//! The rows of facts a probe answers with: `FactLabel` slugs and values already worded for a
//! person, the way `anyview-core`'s `FactValue` words them.

use crate::ffprobe::{Medium, Number, Probed, Stream};
use crate::units::Micros;
use anyview_plugin_protocol::FactRow;

fn row(label: &str, value: impl Into<String>) -> FactRow {
    FactRow {
        label: label.to_owned(),
        value: value.into(),
    }
}

/// The rows for `probed`, in the order to show them: how long, the picture, the sound, the tags,
/// then what the container holds.
pub fn rows(probed: &Probed) -> Vec<FactRow> {
    let mut rows = Vec::new();
    if let Some(length) = probed.duration().filter(|length| length.0 > 0) {
        rows.push(row("duration", clock(length)));
    }
    let video = probed.video();
    if let Some(video) = video {
        if let (Some(w), Some(h)) = (video.width, video.height) {
            rows.push(row("dimensions", format!("{w} × {h}")));
        }
        if let Some(codec) = &video.codec_name {
            rows.push(row("codec", codec.clone()));
        }
        if let Some(rate) = frame_rate(video) {
            rows.push(row("framerate", rate));
        }
    }
    let audio = probed.audio();
    if let Some(audio) = audio {
        if let Some(codec) = &audio.codec_name {
            let label = if video.is_some() {
                "audio_codec"
            } else {
                "codec"
            };
            rows.push(row(label, codec.clone()));
        }
        if let Some(hz) = number(&audio.sample_rate).filter(|hz| *hz > 0.0) {
            rows.push(row("sample_rate", format!("{} kHz", hz / 1000.0)));
        }
        if let Some(channels) = channels(audio) {
            rows.push(row("channels", channels));
        }
    }
    let bits = audio
        .and_then(|audio| number(&audio.bit_rate))
        .or_else(|| number(&probed.format.bit_rate))
        .filter(|bits| *bits > 0.0);
    if let Some(bits) = bits {
        rows.push(row(
            "bitrate",
            format!("{} kbit/s", (bits / 1000.0).round()),
        ));
    }
    for (label, key) in [("title", "title"), ("author", "artist"), ("album", "album")] {
        if let Some(text) = tag(probed, key) {
            rows.push(row(label, text));
        }
    }
    if probed.streams.len() > 1 {
        rows.push(row("streams", streams(probed)));
    }
    if !probed.chapters.is_empty() {
        let n = probed.chapters.len();
        rows.push(row("chapters", format!("{n} {}", plural(n, "chapter"))));
    }
    rows
}

fn number(number: &Option<Number>) -> Option<f64> {
    number.as_ref().and_then(Number::value)
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// `3:07`, or `1:02:03` from one hour; seconds are rounded down.
fn clock(length: Micros) -> String {
    let secs = length.whole_secs();
    let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// `25 fps` or `29.97 fps`, from `30000/1001`; none for a rate of zero or one that is not a
/// fraction.
fn frame_rate(video: &Stream) -> Option<String> {
    let fraction = |text: &Option<String>| {
        let (n, d) = text.as_deref()?.split_once('/')?;
        let (n, d): (u64, u64) = (n.parse().ok()?, d.parse().ok()?);
        (n > 0 && d > 0).then_some((n, d))
    };
    let (n, d) = fraction(&video.avg_frame_rate).or_else(|| fraction(&video.r_frame_rate))?;
    let hundredths = (n * 100 + d / 2) / d;
    let (whole, part) = (hundredths / 100, hundredths % 100);
    let text = match part {
        0 => whole.to_string(),
        p if p % 10 == 0 => format!("{whole}.{}", p / 10),
        p => format!("{whole}.{p:02}"),
    };
    Some(format!("{text} fps"))
}

/// `mono`, `stereo`, `5.1`, or a count when ffprobe names no layout.
fn channels(audio: &Stream) -> Option<String> {
    let count = audio.channels.filter(|count| *count > 0)?;
    Some(match (count, audio.channel_layout.as_deref()) {
        (1, _) => "mono".to_owned(),
        (2, _) => "stereo".to_owned(),
        (_, Some(layout)) if !layout.is_empty() => layout.to_owned(),
        (count, _) => format!("{count} channels"),
    })
}

/// A tag, whatever the case its key is spelled in (Matroska writes `TITLE`).
fn tag(probed: &Probed, key: &str) -> Option<String> {
    let find = |tags: &std::collections::HashMap<String, String>| {
        tags.iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    find(&probed.format.tags).or_else(|| probed.streams.iter().find_map(|s| find(&s.tags)))
}

/// `1 video, 2 audio, 1 subtitle`, and a cover when there is one.
fn streams(probed: &Probed) -> String {
    let count = |wanted: Medium, cover: bool| {
        probed
            .streams
            .iter()
            .filter(|s| s.codec_type == Some(wanted) && (s.disposition.attached_pic != 0) == cover)
            .count()
    };
    let parts = [
        (count(Medium::Video, false), "video"),
        (count(Medium::Audio, false), "audio"),
        (count(Medium::Subtitle, false), "subtitle"),
        (count(Medium::Video, true), "cover"),
    ];
    let words: Vec<String> = parts
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, name)| format!("{n} {name}"))
        .collect();
    if words.is_empty() {
        format!("{} streams", probed.streams.len())
    } else {
        words.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(rows: &[FactRow]) -> Vec<&str> {
        rows.iter().map(|r| r.label.as_str()).collect()
    }

    fn value<'a>(rows: &'a [FactRow], label: &str) -> &'a str {
        rows.iter()
            .find(|r| r.label == label)
            .map_or("", |r| r.value.as_str())
    }

    #[test]
    fn a_video_with_sound_and_tags_has_every_row_in_order() {
        let probed = Probed::parse(
            br#"{"streams":[
              {"index":0,"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001"},
              {"index":1,"codec_type":"audio","codec_name":"aac","sample_rate":"44100","channels":6,"channel_layout":"5.1","bit_rate":"384000"},
              {"index":2,"codec_type":"subtitle"}],
              "chapters":[{},{}],
              "format":{"duration":"3727.5","tags":{"TITLE":"Dune","ARTIST":"Me","album":"A"}}}"#,
        )
        .unwrap();
        let rows = rows(&probed);
        assert_eq!(
            labels(&rows),
            [
                "duration",
                "dimensions",
                "codec",
                "framerate",
                "audio_codec",
                "sample_rate",
                "channels",
                "bitrate",
                "title",
                "author",
                "album",
                "streams",
                "chapters"
            ]
        );
        assert_eq!(value(&rows, "duration"), "1:02:07");
        assert_eq!(value(&rows, "dimensions"), "1920 × 1080");
        assert_eq!(value(&rows, "framerate"), "29.97 fps");
        assert_eq!(value(&rows, "sample_rate"), "44.1 kHz");
        assert_eq!(value(&rows, "channels"), "5.1");
        assert_eq!(value(&rows, "bitrate"), "384 kbit/s");
        assert_eq!(value(&rows, "streams"), "1 video, 1 audio, 1 subtitle");
        assert_eq!(value(&rows, "chapters"), "2 chapters");
    }

    #[test]
    fn a_sound_alone_labels_its_codec_plainly_and_falls_back_to_the_container_bitrate() {
        let probed = Probed::parse(
            br#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"flac","sample_rate":"8000","channels":1}],
              "format":{"duration":"2.0","bit_rate":"63824"}}"#,
        )
        .unwrap();
        let rows = rows(&probed);
        assert_eq!(
            labels(&rows),
            ["duration", "codec", "sample_rate", "channels", "bitrate"]
        );
        assert_eq!(value(&rows, "sample_rate"), "8 kHz");
        assert_eq!(value(&rows, "channels"), "mono");
        assert_eq!(value(&rows, "bitrate"), "64 kbit/s");
    }

    #[test]
    fn frame_rates_round_to_hundredths_and_zero_is_no_rate() {
        let rate = |text: &str| {
            let json = format!(
                r#"{{"index":0,"codec_type":"video","avg_frame_rate":"{text}","r_frame_rate":"0/0"}}"#
            );
            frame_rate(&serde_json::from_str::<Stream>(&json).unwrap())
        };
        assert_eq!(rate("10/1").as_deref(), Some("10 fps"));
        assert_eq!(rate("24000/1001").as_deref(), Some("23.98 fps"));
        assert_eq!(rate("25/2").as_deref(), Some("12.5 fps"));
        assert_eq!(rate("0/0"), None);
    }
}
