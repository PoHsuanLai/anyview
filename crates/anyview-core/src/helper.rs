//! The distro tools the viewer's plugins run, as the viewer names them when one is missing.

use ds_core::word::Word;

/// A tool of the person's own system that a plugin runs, and that the viewer can offer to install
/// when it is not there. The slug of each is the capability's name in the helpers file the
/// viewer ships (`dist/helpers/anyview.toml`), so the file and the code agree by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Helper {
    /// mpv, which plays video and every audio format the built-in player does not.
    VideoPlayback,
    /// FFmpeg and ffprobe, which read a recording's facts and write its exports.
    MediaProbe,
    /// libheif's `heif-dec` (or `heif-convert`), which decodes HEIC pictures.
    HeicDecode,
    /// LibRaw's `dcraw_emu` (or `dcraw`), which develops camera raw files.
    RawDecode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_helper_is_named_as_its_capability_in_the_helpers_file() {
        // helper, capability
        const CASES: &[(Helper, &str)] = &[
            (Helper::VideoPlayback, "video-playback"),
            (Helper::MediaProbe, "media-probe"),
            (Helper::HeicDecode, "heic-decode"),
            (Helper::RawDecode, "raw-decode"),
        ];
        for (helper, capability) in CASES {
            assert_eq!(helper.slug(), *capability, "{helper:?}");
        }
        assert_eq!(Helper::ALL.len(), CASES.len());
    }
}
