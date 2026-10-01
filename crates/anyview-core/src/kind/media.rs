//! Audio and video containers played by the media back end.

use super::FormatKind;
use super::family::Family;
use ds_core::word::Word;

/// A container or stream format for audio or video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum MediaContainer {
    /// MP4 video.
    Mp4,
    /// M4V video.
    M4v,
    /// QuickTime movie.
    Mov,
    /// Matroska video.
    Mkv,
    /// WebM video.
    WebM,
    /// AVI video.
    Avi,
    /// MPEG transport stream video.
    MpegTs,
    /// Ogg video.
    Ogv,
    /// MP3 audio.
    Mp3,
    /// AAC audio.
    Aac,
    /// M4A audio: AAC or ALAC in an MP4 container.
    M4a,
    /// FLAC audio.
    Flac,
    /// WAV audio.
    Wav,
    /// AIFF audio.
    Aiff,
    /// Ogg audio.
    Ogg,
    /// Opus audio.
    Opus,
}

impl Family for MediaContainer {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            MediaContainer::Mp4 => &["mp4"],
            MediaContainer::M4v => &["m4v"],
            MediaContainer::Mov => &["mov", "qt"],
            MediaContainer::Mkv => &["mkv"],
            MediaContainer::WebM => &["webm"],
            MediaContainer::Avi => &["avi"],
            MediaContainer::MpegTs => &["ts", "mts", "m2ts"],
            MediaContainer::Ogv => &["ogv"],
            MediaContainer::Mp3 => &["mp3"],
            MediaContainer::Aac => &["aac"],
            MediaContainer::M4a => &["m4a"],
            MediaContainer::Flac => &["flac"],
            MediaContainer::Wav => &["wav", "wave"],
            MediaContainer::Aiff => &["aiff", "aif", "aifc"],
            MediaContainer::Ogg => &["ogg", "oga"],
            MediaContainer::Opus => &["opus"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            MediaContainer::Mp4 => "video/mp4",
            MediaContainer::M4v => "video/x-m4v",
            MediaContainer::Mov => "video/quicktime",
            MediaContainer::Mkv => "video/x-matroska",
            MediaContainer::WebM => "video/webm",
            MediaContainer::Avi => "video/x-msvideo",
            MediaContainer::MpegTs => "video/mp2t",
            MediaContainer::Ogv => "video/ogg",
            MediaContainer::Mp3 => "audio/mpeg",
            MediaContainer::Aac => "audio/aac",
            MediaContainer::M4a => "audio/mp4",
            MediaContainer::Flac => "audio/flac",
            MediaContainer::Wav => "audio/wav",
            MediaContainer::Aiff => "audio/aiff",
            MediaContainer::Ogg => "audio/ogg",
            MediaContainer::Opus => "audio/opus",
        }
    }
}

impl MediaContainer {
    /// Whether the container is shown as video or played as audio.
    pub fn kind(self) -> FormatKind {
        match self {
            MediaContainer::Mp4
            | MediaContainer::M4v
            | MediaContainer::Mov
            | MediaContainer::Mkv
            | MediaContainer::WebM
            | MediaContainer::Avi
            | MediaContainer::MpegTs
            | MediaContainer::Ogv => FormatKind::Video,
            MediaContainer::Mp3
            | MediaContainer::Aac
            | MediaContainer::M4a
            | MediaContainer::Flac
            | MediaContainer::Wav
            | MediaContainer::Aiff
            | MediaContainer::Ogg
            | MediaContainer::Opus => FormatKind::Audio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_well_formed() {
        crate::kind::family::tests::assert_well_formed::<MediaContainer>();
    }

    #[test]
    fn each_container_is_video_or_audio_by_its_mime_type() {
        const CASES: &[(MediaContainer, FormatKind)] = &[
            (MediaContainer::Mp4, FormatKind::Video),
            (MediaContainer::Mkv, FormatKind::Video),
            (MediaContainer::MpegTs, FormatKind::Video),
            (MediaContainer::Mp3, FormatKind::Audio),
            (MediaContainer::M4a, FormatKind::Audio),
            (MediaContainer::Opus, FormatKind::Audio),
        ];
        for (container, kind) in CASES {
            assert_eq!(container.kind(), *kind, "{container:?}");
        }
        for container in MediaContainer::ALL {
            let prefix = match container.kind() {
                FormatKind::Video => "video/",
                _ => "audio/",
            };
            assert!(container.mime().starts_with(prefix), "{container:?}");
        }
    }
}
