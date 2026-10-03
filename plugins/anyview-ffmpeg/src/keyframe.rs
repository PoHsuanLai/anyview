//! Where a cut of a video can begin: the last keyframe at or before the time asked for, read from
//! the packets alone, so no picture is decoded.

use crate::error::FfmpegError;
use crate::units::Micros;
use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Stdio};

/// How far back from the start each look reaches, in microseconds; the last reaches the
/// beginning of the file.
const WINDOWS: [u64; 2] = [30_000_000, 600_000_000];

#[derive(Debug, Deserialize)]
struct Packets {
    #[serde(default)]
    packets: Vec<Packet>,
}

#[derive(Debug, Deserialize)]
struct Packet {
    pts_time: Option<String>,
    #[serde(default)]
    flags: String,
}

/// The time of the last keyframe of stream `index` at or before `start`, or `start` itself when
/// there is none (the video's first keyframe comes later).
pub fn keyframe_at_or_before(
    ffprobe: &Path,
    input: &Path,
    index: u32,
    start: Micros,
) -> Result<Micros, FfmpegError> {
    for back in WINDOWS.iter().map(|back| Some(*back)).chain([None]) {
        let from = back.map_or(Micros(0), |back| start.minus(Micros(back)));
        let json = packets(ffprobe, input, index, from, start)?;
        if let Some(found) = last_keyframe(&json, start)? {
            return Ok(found);
        }
        if from == Micros(0) {
            break;
        }
    }
    Ok(start)
}

fn packets(
    ffprobe: &Path,
    input: &Path,
    index: u32,
    from: Micros,
    to: Micros,
) -> Result<Vec<u8>, FfmpegError> {
    // The interval is read from the keyframe at or before `from`, so a packet earlier than it
    // may come back; the end is a microsecond late so a keyframe exactly at `start` is read.
    let interval = format!(
        "{}%{}",
        from.ffmpeg_seconds(),
        Micros(to.0 + 1).ffmpeg_seconds()
    );
    let output = Command::new(ffprobe)
        .args(["-v", "error", "-print_format", "json", "-select_streams"])
        .arg(index.to_string())
        .args(["-read_intervals", &interval])
        .args(["-show_entries", "packet=pts_time,flags", "-i"])
        .arg(input)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| FfmpegError::io("run ffprobe", &error))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(FfmpegError::Corrupt(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

/// The latest keyframe at or before `start` among the packets in `json`.
pub fn last_keyframe(json: &[u8], start: Micros) -> Result<Option<Micros>, FfmpegError> {
    let packets: Packets =
        serde_json::from_slice(json).map_err(|error| FfmpegError::Corrupt(error.to_string()))?;
    Ok(packets
        .packets
        .iter()
        .filter(|packet| packet.flags.contains('K'))
        .filter_map(|packet| packet.pts_time.as_deref()?.parse::<f64>().ok())
        .filter_map(Micros::from_secs_f64)
        .filter(|time| *time <= start)
        .max())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_keyframe_not_after_the_start_is_chosen() {
        let json = br#"{"packets":[
            {"pts_time":"0.032000","flags":"K__"},
            {"pts_time":"0.132000","flags":"___"},
            {"pts_time":"1.232000","flags":"K__"},
            {"pts_time":"1.332000","flags":"___"},
            {"pts_time":"2.432000","flags":"K__"},
            {"flags":"K__"}]}"#;
        assert_eq!(
            last_keyframe(json, Micros(1_500_000)).unwrap(),
            Some(Micros(1_232_000))
        );
        assert_eq!(
            last_keyframe(json, Micros(1_232_000)).unwrap(),
            Some(Micros(1_232_000))
        );
        assert_eq!(last_keyframe(json, Micros(10_000)).unwrap(), None);
        assert_eq!(last_keyframe(br#"{}"#, Micros(5)).unwrap(), None);
        assert!(last_keyframe(b"nope", Micros(5)).is_err());
    }
}
