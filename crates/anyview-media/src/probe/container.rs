//! Which container a libav format name stands for.

use anyview_core::MediaContainer;

/// The container named by libav's format `name` (a comma-separated list, as libav writes it:
/// `matroska,webm`), told apart by the file's `extension` where one name covers several. `None`
/// when the name is not one the viewer lists.
pub(super) fn container_of(name: &str, extension: Option<&str>) -> Option<MediaContainer> {
    let names: Vec<&str> = name.split(',').collect();
    let has = |wanted: &str| names.contains(&wanted);
    let by_extension = |candidates: &[MediaContainer]| {
        candidates
            .iter()
            .copied()
            .find(|container| {
                extension.is_some_and(|ext| container.extension().eq_ignore_ascii_case(ext))
            })
            .or_else(|| candidates.first().copied())
    };
    if has("matroska") {
        by_extension(&[MediaContainer::Mkv, MediaContainer::WebM])
    } else if has("mov") || has("mp4") {
        by_extension(&[
            MediaContainer::Mp4,
            MediaContainer::M4a,
            MediaContainer::M4v,
            MediaContainer::Mov,
        ])
    } else if has("ogg") {
        by_extension(&[
            MediaContainer::Ogg,
            MediaContainer::Opus,
            MediaContainer::Ogv,
        ])
    } else if has("mpegts") {
        Some(MediaContainer::MpegTs)
    } else if has("mpeg") || has("mpegvideo") {
        Some(MediaContainer::Mpeg)
    } else if has("avi") {
        Some(MediaContainer::Avi)
    } else if has("asf") {
        Some(MediaContainer::Wmv)
    } else if has("flv") {
        Some(MediaContainer::Flv)
    } else if has("mp3") {
        Some(MediaContainer::Mp3)
    } else if has("aac") {
        Some(MediaContainer::Aac)
    } else if has("flac") {
        Some(MediaContainer::Flac)
    } else if has("wav") {
        Some(MediaContainer::Wav)
    } else if has("aiff") {
        Some(MediaContainer::Aiff)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_name_is_a_container_told_apart_by_extension() {
        const CASES: &[(&str, &str, Option<&str>, Option<MediaContainer>)] = &[
            (
                "mkv",
                "matroska,webm",
                Some("mkv"),
                Some(MediaContainer::Mkv),
            ),
            (
                "webm",
                "matroska,webm",
                Some("webm"),
                Some(MediaContainer::WebM),
            ),
            (
                "matroska without a hint",
                "matroska,webm",
                None,
                Some(MediaContainer::Mkv),
            ),
            (
                "mp4",
                "mov,mp4,m4a,3gp,3g2,mj2",
                Some("mp4"),
                Some(MediaContainer::Mp4),
            ),
            (
                "m4a",
                "mov,mp4,m4a,3gp,3g2,mj2",
                Some("M4A"),
                Some(MediaContainer::M4a),
            ),
            (
                "mov",
                "mov,mp4,m4a,3gp,3g2,mj2",
                Some("mov"),
                Some(MediaContainer::Mov),
            ),
            (
                "opus in ogg",
                "ogg",
                Some("opus"),
                Some(MediaContainer::Opus),
            ),
            ("ogg", "ogg", Some("ogg"), Some(MediaContainer::Ogg)),
            ("flac", "flac", Some("flac"), Some(MediaContainer::Flac)),
            ("mp3", "mp3", None, Some(MediaContainer::Mp3)),
            ("wav", "wav", None, Some(MediaContainer::Wav)),
            ("unknown", "image2", None, None),
        ];
        for (name, format, extension, want) in CASES {
            assert_eq!(container_of(format, *extension), *want, "{name}");
        }
    }
}
