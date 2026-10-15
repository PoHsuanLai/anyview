//! The arguments of an export, decided from the request and what the file holds. Pure: it starts
//! nothing, so each target has a row in the tests.

use crate::encoders::Encoders;
use crate::error::FfmpegError;
use crate::ffprobe::{Medium, Number, Probed, Stream};
use crate::target::Target;
use crate::units::Micros;
use anyview_plugin_protocol::ExportRequest;
use std::ffi::OsString;
use std::path::Path;

/// The bitrate of a lossy target when the request names none: 192 kbit/s, as the viewer's own
/// default is.
const DEFAULT_BITS: u32 = 192_000;

/// The sample rates LAME encodes at; a source at another is written at 44.1 kHz.
const MP3_RATES: [u32; 9] = [
    8000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
];

/// Where a cut begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    /// The time the output's first picture or sound has in the source.
    pub origin: Micros,
    /// What `-ss` is given: the origin with a microsecond's margin when the origin is a keyframe,
    /// so rounding in a timestamp cannot make FFmpeg drop the keyframe it was sent to find.
    pub seek: Micros,
}

impl Cut {
    /// A cut that begins exactly at `at`.
    pub fn at(at: Micros) -> Cut {
        Cut {
            origin: at,
            seek: at,
        }
    }

    /// A cut that begins on the keyframe at `key`.
    pub fn on_keyframe(key: Micros) -> Cut {
        Cut {
            origin: key,
            seek: key.minus(Micros(1)),
        }
    }
}

/// How a cut written over the recording itself writes its sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// The packets are copied: the cut is as exact as they are. A recording with pictures is cut
    /// at a keyframe whatever is done to its sound, and a sound that is already PCM is cut to
    /// the sample by copying.
    Copy,
    /// The sound is decoded and written again with the codec it had, at the rate it had, so the
    /// cut falls on the sample it was asked for and the quality is the one the file had.
    Encode {
        /// The FFmpeg encoder of the sound's codec.
        encoder: &'static str,
        /// Bits a second the file had; none for a lossless codec, or when the file did not say.
        bits: Option<u32>,
    },
}

/// Whether writing a codec again keeps a bitrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Loss {
    /// The codec loses detail and the file's rate is kept.
    Lossy,
    /// The codec keeps every sample, so it has no rate to keep.
    Lossless,
}

/// The encoders that write `codec` again, best first, and whether it loses detail. PCM is absent:
/// copying it is exact. So is a codec FFmpeg has no encoder for here, which is copied.
fn encoders_of(codec: &str) -> Option<(&'static [&'static str], Loss)> {
    match codec {
        "aac" => Some((&["aac"], Loss::Lossy)),
        "mp3" => Some((&["libmp3lame"], Loss::Lossy)),
        "opus" => Some((&["libopus"], Loss::Lossy)),
        "vorbis" => Some((&["libvorbis"], Loss::Lossy)),
        "flac" => Some((&["flac"], Loss::Lossless)),
        "alac" => Some((&["alac"], Loss::Lossless)),
        _ => None,
    }
}

/// How a cut of the recording `probed` writes its sound: copied when that is exact (a recording
/// with pictures, which is cut at a keyframe, and PCM), otherwise written again with its own
/// codec when `encoders` has one. A recording whose codec cannot be written again is copied.
pub fn sound_of_cut(probed: &Probed, encoders: &Encoders) -> Sound {
    if probed.video().is_some() {
        return Sound::Copy;
    }
    let Some(audio) = probed.audio() else {
        return Sound::Copy;
    };
    let Some((names, loss)) = audio.codec_name.as_deref().and_then(encoders_of) else {
        return Sound::Copy;
    };
    let Some(encoder) = encoders.first_of(names) else {
        return Sound::Copy;
    };
    let bits = match loss {
        Loss::Lossless => None,
        Loss::Lossy => audio
            .bit_rate
            .as_ref()
            .or(probed.format.bit_rate.as_ref())
            .and_then(Number::value)
            .filter(|rate| *rate >= 8_000.0)
            .map(|rate| rate.round() as u32),
    };
    Sound::Encode { encoder, bits }
}

/// What to run and how much work it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Everything after the program's name.
    pub args: Vec<OsString>,
    /// How much of the recording is written, in microseconds; zero when it is not known.
    pub total: Micros,
}

/// The audio stream a target reads: the one the request names, else the one FFmpeg would pick.
fn audio_source<'a>(
    request: &ExportRequest,
    probed: &'a Probed,
) -> Result<&'a Stream, FfmpegError> {
    match request.stream {
        Some(index) => probed
            .stream(index)
            .filter(|stream| stream.codec_type == Some(Medium::Audio))
            .ok_or_else(|| FfmpegError::Unsupported(format!("stream {index} is not audio"))),
        None => probed
            .audio()
            .ok_or_else(|| FfmpegError::Unsupported("the file has no audio track".to_owned())),
    }
}

/// The range of the request: where it starts and where it ends, in the source.
pub fn range_of(request: &ExportRequest) -> Result<(Micros, Option<Micros>), FfmpegError> {
    let Some(range) = request.range else {
        return Ok((Micros(0), None));
    };
    if range.end.is_some_and(|end| end <= range.start) {
        return Err(FfmpegError::Unsupported("the range is empty".to_owned()));
    }
    Ok((Micros(range.start), range.end.map(Micros)))
}

/// The arguments that write `request` as `target` to `temp`, starting at `cut`.
pub fn plan(
    request: &ExportRequest,
    target: Target,
    encoders: &Encoders,
    probed: &Probed,
    cut: Cut,
    temp: &Path,
) -> Result<Plan, FfmpegError> {
    let (_, end) = range_of(request)?;
    let mut args: Vec<OsString> = Vec::new();
    let mut push = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    push(&[
        "-v",
        "error",
        "-nostdin",
        "-nostats",
        "-progress",
        "pipe:1",
        "-y",
    ]);
    if cut.seek > Micros(0) {
        push(&["-ss", &cut.seek.ffmpeg_seconds()]);
    }
    args.push("-i".into());
    args.push(request.input.clone().into_os_string());
    let mut push = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    if let Some(end) = end {
        push(&["-t", &end.minus(cut.origin).ffmpeg_seconds()]);
    }
    match target {
        Target::Trim => match request.stream {
            Some(index) => push(&["-map", &format!("0:{index}"), "-c", "copy"]),
            None => push(&["-map", "0:V?", "-map", "0:a?", "-map", "0:s?", "-c", "copy"]),
        },
        Target::CutInPlace => {
            push(&["-map", "0:v?", "-map", "0:a?", "-map", "0:s?", "-c", "copy"]);
            if let Sound::Encode { encoder, bits } = sound_of_cut(probed, encoders) {
                push(&["-c:a", encoder]);
                if let Some(bits) = bits {
                    push(&["-b:a", &bits.to_string()]);
                }
            }
        }
        Target::AudioCopy => {
            let source = audio_source(request, probed)?;
            push(&["-map", &format!("0:{}", source.index), "-c", "copy"]);
        }
        Target::M4a | Target::Mp3 | Target::Flac | Target::Wav | Target::Opus => {
            let source = audio_source(request, probed)?;
            let encoder = encoders.for_target(target).ok_or_else(|| {
                FfmpegError::Unsupported(format!(
                    "this FFmpeg has no encoder for {}",
                    target.slug()
                ))
            })?;
            push(&["-map", &format!("0:{}", source.index)]);
            push(&["-c:a", encoder]);
            let bits = request.bitrate.unwrap_or(DEFAULT_BITS).to_string();
            let rate = source
                .sample_rate
                .as_ref()
                .and_then(|n| n.value())
                .map_or(0, |hz| hz as u32);
            let downmix = source.channels.is_some_and(|channels| channels > 2);
            match target {
                Target::M4a => push(&["-b:a", &bits]),
                Target::Mp3 => {
                    push(&["-b:a", &bits]);
                    if !MP3_RATES.contains(&rate) {
                        push(&["-ar", "44100"]);
                    }
                }
                Target::Flac => push(&["-sample_fmt", "s16"]),
                Target::Opus => {
                    push(&["-b:a", &bits, "-ar", "48000"]);
                    if encoder == "opus" {
                        push(&["-strict", "-2"]);
                    }
                }
                Target::Wav | Target::Trim | Target::CutInPlace | Target::AudioCopy => {}
            }
            let lossy = matches!(target, Target::M4a | Target::Mp3 | Target::Opus);
            if lossy && downmix {
                push(&["-ac", "2"]);
            }
        }
    }
    match target {
        Target::Trim | Target::CutInPlace => push(&["-copypriorss", "0"]),
        Target::AudioCopy
        | Target::M4a
        | Target::Mp3
        | Target::Flac
        | Target::Wav
        | Target::Opus => push(&["-vn", "-sn", "-dn"]),
    }
    push(&["-map_metadata", "0"]);
    // A recording made to be shared must not say where it was shot: blank every tag the
    // containers keep a position in (an empty value removes the tag). The recording itself,
    // made shorter, keeps every tag it had.
    if target != Target::CutInPlace {
        for key in LOCATION_TAGS {
            push(&["-metadata", &format!("{key}=")]);
        }
    }
    args.push(temp.as_os_str().to_owned());
    let whole = end.or_else(|| probed.duration());
    Ok(Plan {
        args,
        total: whole.map_or(Micros(0), |whole| whole.minus(cut.origin)),
    })
}

/// The metadata keys that hold a position: MP4's and QuickTime's `location`, its language-tagged
/// form, and the keys Apple's cameras and Android write.
const LOCATION_TAGS: [&str; 4] = [
    "location",
    "location-eng",
    "com.apple.quicktime.location.ISO6709",
    "com.android.location",
];

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_plugin_protocol::MicroRange;

    const FILE: &str = r#"{"streams":[
        {"index":0,"codec_type":"video","codec_name":"mpeg4","width":64,"height":48},
        {"index":1,"codec_type":"audio","codec_name":"vorbis","sample_rate":"8000","channels":1},
        {"index":2,"codec_type":"audio","codec_name":"ac3","sample_rate":"48000","channels":6},
        {"index":3,"codec_type":"subtitle"}],
        "format":{"duration":"3.0"}}"#;

    fn request(target: &str) -> ExportRequest {
        ExportRequest {
            input: "/in.mkv".into(),
            output: "/out.x".into(),
            target: target.to_owned(),
            range: None,
            stream: None,
            bitrate: None,
        }
    }

    fn line(plan: &Plan) -> String {
        plan.args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn everything() -> Encoders {
        Encoders::parse(
            "---\n A....D aac x\n A....D libmp3lame x\n A....D flac x\n A....D pcm_s16le x\n A....D libopus x\n",
        )
    }

    fn make(request: &ExportRequest, target: Target, cut: Cut) -> Result<Plan, FfmpegError> {
        let probed = Probed::parse(FILE.as_bytes()).unwrap();
        plan(
            request,
            target,
            &everything(),
            &probed,
            cut,
            Path::new("/.part-out.x"),
        )
    }

    #[test]
    fn a_trim_seeks_before_the_input_copies_every_stream_and_drops_what_precedes_the_keyframe() {
        let mut asked = request("trim");
        asked.range = Some(MicroRange {
            start: 1_500_000,
            end: Some(2_500_000),
        });
        let planned = make(&asked, Target::Trim, Cut::on_keyframe(Micros(1_232_000))).unwrap();
        let text = line(&planned);
        assert!(
            text.contains("-ss 1.231999 -i /in.mkv -t 1.268000 -map 0:V? -map 0:a? -map 0:s? -c copy -copypriorss 0"),
            "{text}"
        );
        assert!(
            text.contains("-map_metadata 0 -metadata location= "),
            "{text}"
        );
        assert!(text.ends_with(" /.part-out.x"), "{text}");
        assert_eq!(planned.total, Micros(1_268_000));
    }

    #[test]
    fn every_export_blanks_the_location_tags_after_carrying_the_rest() {
        for (name, target) in [("trim", Target::Trim), ("m4a", Target::M4a)] {
            let text = line(&make(&request(name), target, Cut::at(Micros(0))).unwrap());
            let carried = text.find("-map_metadata 0").unwrap();
            for key in LOCATION_TAGS {
                let blanked = text.find(&format!("-metadata {key}= ")).unwrap_or(0);
                assert!(blanked > carried, "{name} blanks {key}: {text}");
            }
        }
    }

    #[test]
    fn a_whole_recording_has_no_seek_and_its_length_is_the_total() {
        let planned = make(&request("trim"), Target::Trim, Cut::at(Micros(0))).unwrap();
        let text = line(&planned);
        assert!(!text.contains("-ss") && !text.contains("-t "), "{text}");
        assert_eq!(planned.total, Micros(3_000_000));
    }

    #[test]
    fn audio_targets_read_the_best_audio_track_and_drop_the_rest() {
        let asked = |target: &str, bitrate| ExportRequest {
            bitrate,
            ..request(target)
        };
        let cases: Vec<(&str, Target, Option<u32>, &str)> = vec![
            (
                "copy",
                Target::AudioCopy,
                None,
                "-map 0:2 -c copy -vn -sn -dn",
            ),
            (
                "m4a",
                Target::M4a,
                None,
                "-map 0:2 -c:a aac -b:a 192000 -ac 2 -vn",
            ),
            (
                "mp3",
                Target::Mp3,
                Some(128_000),
                "-c:a libmp3lame -b:a 128000 -ac 2",
            ),
            ("flac", Target::Flac, None, "-c:a flac -sample_fmt s16 -vn"),
            ("wav", Target::Wav, None, "-c:a pcm_s16le -vn"),
            (
                "opus",
                Target::Opus,
                None,
                "-c:a libopus -b:a 192000 -ar 48000 -ac 2",
            ),
        ];
        for (name, target, bitrate, want) in cases {
            let text =
                line(&make(&asked(target.slug(), bitrate), target, Cut::at(Micros(0))).unwrap());
            assert!(text.contains(want), "{name}: {text}");
        }
    }

    #[test]
    fn a_named_stream_is_the_one_kept_and_must_be_audio_for_an_audio_target() {
        let mut asked = request("flac");
        asked.stream = Some(1);
        assert!(
            line(&make(&asked, Target::Flac, Cut::at(Micros(0))).unwrap())
                .contains("-map 0:1 -c:a flac")
        );
        asked.stream = Some(0);
        assert!(matches!(
            make(&asked, Target::Flac, Cut::at(Micros(0))),
            Err(FfmpegError::Unsupported(_))
        ));
    }

    #[test]
    fn mp3_keeps_a_rate_lame_has_and_resamples_another() {
        let probed = Probed::parse(
            br#"{"streams":[{"index":0,"codec_type":"audio","sample_rate":"96000","channels":2}]}"#,
        )
        .unwrap();
        let planned = plan(
            &request("mp3"),
            Target::Mp3,
            &everything(),
            &probed,
            Cut::at(Micros(0)),
            Path::new("/o"),
        )
        .unwrap();
        assert!(line(&planned).contains("-ar 44100"));
        assert!(!line(&planned).contains("-ac"), "stereo stays");
    }

    #[test]
    fn a_range_with_no_end_runs_to_the_end_of_the_recording() {
        let mut asked = request("trim");
        asked.range = Some(MicroRange {
            start: 1_500_000,
            end: None,
        });
        let planned = make(&asked, Target::Trim, Cut::at(Micros(1_500_000))).unwrap();
        let text = line(&planned);
        assert!(text.contains("-ss 1.500000 -i /in.mkv -map"), "{text}");
        assert!(!text.contains(" -t "), "{text}");
        assert_eq!(planned.total, Micros(1_500_000), "3 s file, 1.5 s kept");
    }

    fn audio_file(codec: &str, extra: &str) -> String {
        format!(
            r#"{{"streams":[{{"index":0,"codec_type":"audio","codec_name":"{codec}","sample_rate":"44100","channels":2{extra}}}],
            "format":{{"duration":"3.0"}}}}"#
        )
    }

    #[test]
    fn a_cut_copies_what_is_exact_and_writes_the_rest_again_in_its_own_codec() {
        let with_cover = r#"{"streams":[
            {"index":0,"codec_type":"audio","codec_name":"mp3","bit_rate":"192000"},
            {"index":1,"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}}]}"#;
        let container_rate = r#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"aac"}],
            "format":{"bit_rate":"128000"}}"#;
        let flac = audio_file("flac", r#","bit_rate":"900000""#);
        let mp3 = audio_file("mp3", r#","bit_rate":"192000""#);
        let pcm = audio_file("pcm_s16le", "");
        let wma = audio_file("wmav2", r#","bit_rate":"128000""#);
        // name, what ffprobe says, the cut's sound
        let cases: Vec<(&str, &str, Sound)> = vec![
            (
                "a recording with pictures is cut at a keyframe",
                FILE,
                Sound::Copy,
            ),
            (
                "lossy sound keeps the rate it had",
                mp3.as_str(),
                Sound::Encode {
                    encoder: "libmp3lame",
                    bits: Some(192_000),
                },
            ),
            (
                "lossless sound has no rate to keep",
                flac.as_str(),
                Sound::Encode {
                    encoder: "flac",
                    bits: None,
                },
            ),
            (
                "PCM is cut to the sample by copying",
                pcm.as_str(),
                Sound::Copy,
            ),
            (
                "a codec with no encoder here is copied",
                wma.as_str(),
                Sound::Copy,
            ),
            (
                "the container's rate stands in for a stream that gives none",
                container_rate,
                Sound::Encode {
                    encoder: "aac",
                    bits: Some(128_000),
                },
            ),
            (
                "cover art does not make a sound a recording with pictures",
                with_cover,
                Sound::Encode {
                    encoder: "libmp3lame",
                    bits: Some(192_000),
                },
            ),
        ];
        for (name, json, want) in cases {
            let probed = Probed::parse(json.as_bytes()).unwrap();
            assert_eq!(sound_of_cut(&probed, &everything()), want, "{name}");
        }
        let mp3 = Probed::parse(audio_file("mp3", "").as_bytes()).unwrap();
        assert_eq!(
            sound_of_cut(&mp3, &Encoders::default()),
            Sound::Copy,
            "no encoder, so a copy"
        );
    }

    #[test]
    fn a_cut_keeps_every_stream_and_tag_and_asks_only_the_sound_to_be_written_again() {
        let mut asked = request("cut");
        asked.range = Some(MicroRange {
            start: 1_000_000,
            end: Some(2_000_000),
        });
        let plan_for = |json: &str| {
            let probed = Probed::parse(json.as_bytes()).unwrap();
            let planned = plan(
                &asked,
                Target::CutInPlace,
                &everything(),
                &probed,
                Cut::at(Micros(1_000_000)),
                Path::new("/.part-out.mp3"),
            )
            .unwrap();
            line(&planned)
        };
        let sound = plan_for(&audio_file("mp3", r#","bit_rate":"192000""#));
        assert!(
            sound.contains(
                "-ss 1.000000 -i /in.mkv -t 1.000000 -map 0:v? -map 0:a? -map 0:s? -c copy -c:a libmp3lame -b:a 192000 -copypriorss 0"
            ),
            "{sound}"
        );
        let video = plan_for(FILE);
        assert!(video.contains("-c copy -copypriorss 0"), "{video}");
        assert!(!video.contains("-c:a"), "{video}");
        for text in [&sound, &video] {
            assert!(text.contains("-map_metadata 0"), "{text}");
            assert!(!text.contains("-metadata location="), "{text}");
        }
    }

    #[test]
    fn what_cannot_be_done_is_refused_before_ffmpeg_runs() {
        let mut empty = request("trim");
        empty.range = Some(MicroRange {
            start: 5,
            end: Some(5),
        });
        assert!(matches!(
            make(&empty, Target::Trim, Cut::at(Micros(5))),
            Err(FfmpegError::Unsupported(_))
        ));
        let silent = Probed::parse(br#"{"streams":[{"index":0,"codec_type":"video"}]}"#).unwrap();
        let none = plan(
            &request("mp3"),
            Target::Mp3,
            &everything(),
            &silent,
            Cut::at(Micros(0)),
            Path::new("/o"),
        );
        assert!(matches!(none, Err(FfmpegError::Unsupported(_))));
        let probed = Probed::parse(FILE.as_bytes()).unwrap();
        let no_encoder = plan(
            &request("mp3"),
            Target::Mp3,
            &Encoders::default(),
            &probed,
            Cut::at(Micros(0)),
            Path::new("/o"),
        );
        assert!(matches!(no_encoder, Err(FfmpegError::Unsupported(_))));
    }
}
