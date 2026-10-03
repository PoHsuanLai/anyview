//! A picture of a recording: one frame, or the cover of a sound, scaled and read back as raw RGBA.

use crate::error::FfmpegError;
use crate::ffprobe::{Probed, Stream};
use crate::units::Micros;
use std::path::Path;
use std::process::{Command, Stdio};

/// The most of a recording a representative frame waits for.
const FRAME_AT_MOST: Micros = Micros(5_000_000);

/// What the picture may measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// No side longer than this many pixels.
    Edge(u32),
    /// No more than this many pixels in all.
    Area(u64),
}

/// A picture as straight RGBA8 rows from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// The size the picture of `stream` has when it is shown: the pixel's shape applied and a
/// quarter turn swapping the sides.
fn shown_size(stream: &Stream) -> Option<(u32, u32)> {
    let (w, h) = (stream.width?, stream.height?);
    if w == 0 || h == 0 {
        return None;
    }
    let aspect = stream
        .sample_aspect_ratio
        .as_deref()
        .and_then(|text| text.split_once(':'))
        .and_then(|(n, d)| Some((n.parse::<u64>().ok()?, d.parse::<u64>().ok()?)))
        .filter(|(n, d)| *n > 0 && *d > 0);
    let w = match aspect {
        Some((n, d)) => u32::try_from((u64::from(w) * n + d / 2) / d)
            .unwrap_or(w)
            .max(1),
        None => w,
    };
    Some(if stream.rotation().rem_euclid(180) == 90 {
        (h, w)
    } else {
        (w, h)
    })
}

/// The size within `fit` that keeps the shape of `size` and never enlarges it.
pub fn fitted(size: (u32, u32), fit: Fit) -> (u32, u32) {
    let (w, h) = (u64::from(size.0), u64::from(size.1));
    let scaled = |scale: f64| {
        let side = |n: u64| ((n as f64 * scale).floor() as u64).max(1);
        (side(w), side(h))
    };
    let (nw, nh) = match fit {
        Fit::Edge(edge) => {
            let longer = w.max(h);
            if longer <= u64::from(edge) {
                (w, h)
            } else {
                scaled(f64::from(edge) / longer as f64)
            }
        }
        Fit::Area(area) => {
            if w * h <= area {
                (w, h)
            } else {
                let (mut nw, mut nh) = scaled((area as f64 / (w * h) as f64).sqrt());
                while nw * nh > area && (nw > 1 || nh > 1) {
                    if nw > 1 {
                        nw -= 1;
                    } else {
                        nh -= 1;
                    }
                }
                (nw, nh)
            }
        }
    };
    (
        u32::try_from(nw).unwrap_or(u32::MAX),
        u32::try_from(nh).unwrap_or(u32::MAX),
    )
}

/// Where in the recording a representative frame is: a tenth of the way in, at most five seconds.
pub fn frame_time(duration: Option<Micros>) -> Micros {
    duration.map_or(Micros(0), |length| {
        Micros((length.0 / 10).min(FRAME_AT_MOST.0))
    })
}

/// Which stream gives the picture and where in it: a moving picture a little way in, else the
/// cover, which has no time.
fn source(probed: &Probed) -> Result<(&Stream, Micros), FfmpegError> {
    if let Some(video) = probed.video() {
        let length = probed.duration();
        return Ok((video, frame_time(length)));
    }
    probed
        .cover()
        .map(|cover| (cover, Micros(0)))
        .ok_or_else(|| FfmpegError::Unsupported("the file has no picture to show".to_owned()))
}

/// The picture of the file at `input` within `fit`.
pub fn picture(
    ffmpeg: &Path,
    input: &Path,
    probed: &Probed,
    fit: Fit,
) -> Result<Picture, FfmpegError> {
    let (stream, at) = source(probed)?;
    let shown = shown_size(stream)
        .ok_or_else(|| FfmpegError::Unsupported("the picture has no size".to_owned()))?;
    let (width, height) = fitted(shown, fit);
    let wanted = u64::from(width) * u64::from(height) * 4;
    let mut rgba = frame(ffmpeg, input, stream.index, at, (width, height))?;
    // A recording shorter than the time asked for has no frame there: the first one will do.
    if u64::try_from(rgba.len()).unwrap_or(u64::MAX) != wanted && at > Micros(0) {
        rgba = frame(ffmpeg, input, stream.index, Micros(0), (width, height))?;
    }
    if u64::try_from(rgba.len()).unwrap_or(u64::MAX) != wanted {
        return Err(FfmpegError::Failed(format!(
            "ffmpeg wrote {} bytes for a {width} x {height} picture",
            rgba.len()
        )));
    }
    Ok(Picture {
        width,
        height,
        rgba,
    })
}

fn frame(
    ffmpeg: &Path,
    input: &Path,
    index: u32,
    at: Micros,
    (width, height): (u32, u32),
) -> Result<Vec<u8>, FfmpegError> {
    let mut command = Command::new(ffmpeg);
    command.args(["-v", "error", "-nostdin"]);
    if at > Micros(0) {
        command.arg("-ss").arg(at.ffmpeg_seconds());
    }
    let output = command
        .arg("-i")
        .arg(input)
        .arg("-map")
        .arg(format!("0:{index}"))
        .args(["-an", "-sn", "-dn", "-frames:v", "1", "-vf"])
        .arg(format!("scale={width}:{height}:flags=bicubic,setsar=1"))
        .args(["-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1"])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| FfmpegError::io("run ffmpeg", &error))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(FfmpegError::Failed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_fitted_without_being_enlarged() {
        const CASES: &[(&str, (u32, u32), Fit, (u32, u32))] = &[
            ("small stays", (64, 48), Fit::Edge(256), (64, 48)),
            ("wide", (1920, 1080), Fit::Edge(256), (256, 144)),
            ("tall", (1080, 1920), Fit::Edge(256), (144, 256)),
            ("area allows", (64, 48), Fit::Area(10_000), (64, 48)),
            (
                "area shrinks",
                (6000, 4000),
                Fit::Area(1_000_000),
                (1224, 816),
            ),
            ("a sliver", (10_000, 1), Fit::Area(100), (100, 1)),
        ];
        for (name, size, fit, want) in CASES {
            let got = fitted(*size, *fit);
            assert_eq!(got, *want, "{name}");
            if let Fit::Area(area) = fit {
                assert!(u64::from(got.0) * u64::from(got.1) <= *area, "{name}");
            }
        }
    }

    #[test]
    fn the_frame_is_a_tenth_in_and_at_most_five_seconds() {
        assert_eq!(frame_time(None), Micros(0));
        assert_eq!(frame_time(Some(Micros(3_032_000))), Micros(303_200));
        assert_eq!(frame_time(Some(Micros(7_200_000_000))), Micros(5_000_000));
    }

    #[test]
    fn a_turned_or_stretched_picture_is_measured_as_shown() {
        let stream = |json: &str| serde_json::from_str::<Stream>(json).unwrap();
        let turned =
            stream(r#"{"index":0,"width":1920,"height":1080,"side_data_list":[{"rotation":-90}]}"#);
        assert_eq!(shown_size(&turned), Some((1080, 1920)));
        let anamorphic =
            stream(r#"{"index":0,"width":720,"height":576,"sample_aspect_ratio":"16:15"}"#);
        assert_eq!(shown_size(&anamorphic), Some((768, 576)));
        assert_eq!(shown_size(&stream(r#"{"index":0}"#)), None);
    }
}
