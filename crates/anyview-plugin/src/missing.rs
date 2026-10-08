//! The package that would serve a kind when no installed plugin does.

use crate::handles::Subject;
use anyview_core::{Fact, FactLabel, FactValue, FormatKind, Helper};
use anyview_plugin_protocol::Capability;
use bayonet::{Suggestion, suggest};

/// A package a person installs with their system's package manager. Its helper is the distro tool
/// the package's plugin runs: what the viewer offers to install when the plugin is there and its
/// tool is not. Its tool is the file name of the program the plugin's manifest names, when it names
/// one the system provides (mpv's), which is then visible in the manifest as a missing file.
pub type Package = bayonet::Package<Helper>;

/// Playback, through the user's own mpv.
const MPV: Package = Package::new("anyview-mpv", Helper::VideoPlayback, Some("mpv"));
/// Facts, pictures and conversions, through the user's own FFmpeg.
const FFMPEG: Package = Package::new("anyview-ffmpeg", Helper::MediaProbe, None);

/// HEIC and HEIF pictures (and AVIF when the viewer has no decoder of its own), through the
/// person's own libheif tools.
const HEIF: Package = Package::new("anyview-heif", Helper::HeicDecode, None);
/// Camera raw files developed in full, through the person's own LibRaw tools.
const RAW: Package = Package::new("anyview-raw", Helper::RawDecode, None);

/// What a row of [`SUGGESTIONS`] matches: a kind and, when it names a media type, that type.
#[derive(Debug, Clone, Copy)]
struct Wants {
    kind: FormatKind,
    mime: Option<&'static str>,
}

/// One row of [`SUGGESTIONS`].
const fn row(
    capability: Capability,
    kind: FormatKind,
    mime: Option<&'static str>,
    package: Package,
) -> Suggestion<Capability, Wants, Helper> {
    Suggestion {
        capability,
        key: Wants { kind, mime },
        package,
    }
}

/// Which package provides a capability for a kind. A row matches when its capability and its kind
/// are the ones asked about and, when the row names a media type, the file's media type is that
/// one: a raster image is not one plugin's business, HEIC is.
const SUGGESTIONS: &[Suggestion<Capability, Wants, Helper>] = &[
    row(Capability::Probe, FormatKind::Video, None, FFMPEG),
    row(Capability::Probe, FormatKind::Audio, None, FFMPEG),
    row(Capability::Peek, FormatKind::Video, None, FFMPEG),
    row(Capability::Peek, FormatKind::Audio, None, FFMPEG),
    row(Capability::Thumbnail, FormatKind::Video, None, FFMPEG),
    row(Capability::Thumbnail, FormatKind::Audio, None, FFMPEG),
    row(Capability::Decode, FormatKind::Video, None, FFMPEG),
    row(Capability::Export, FormatKind::Video, None, FFMPEG),
    row(Capability::Export, FormatKind::Audio, None, FFMPEG),
    row(Capability::Play, FormatKind::Video, None, MPV),
    row(Capability::Play, FormatKind::Audio, None, MPV),
    row(
        Capability::Thumbnail,
        FormatKind::Raster,
        Some("image/heic"),
        HEIF,
    ),
    row(
        Capability::Thumbnail,
        FormatKind::Raster,
        Some("image/avif"),
        HEIF,
    ),
    row(
        Capability::Decode,
        FormatKind::Raster,
        Some("image/heic"),
        HEIF,
    ),
    row(
        Capability::Decode,
        FormatKind::Raster,
        Some("image/avif"),
        HEIF,
    ),
    row(
        Capability::Thumbnail,
        FormatKind::Raster,
        Some("image/x-dcraw"),
        RAW,
    ),
    row(
        Capability::Decode,
        FormatKind::Raster,
        Some("image/x-dcraw"),
        RAW,
    ),
];

/// The package to suggest for `capability` on `subject`, or `None` when no package is known.
pub fn suggested_package(capability: Capability, subject: &Subject<'_>) -> Option<Package> {
    suggest(SUGGESTIONS, capability, |wants| {
        wants.kind == subject.kind
            && wants
                .mime
                .is_none_or(|wanted| subject.mime.is_some_and(|m| m.as_str() == wanted))
    })
}

/// A kind needs a capability that no installed plugin provides, and a package that would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingPlugin {
    /// What is needed.
    pub capability: Capability,
    /// For which kind of file.
    pub kind: FormatKind,
    /// What provides it.
    pub package: Package,
}

impl MissingPlugin {
    /// The row a facts card lists: `Needs`, then the package and what it is for.
    pub fn fact(&self) -> Fact {
        self.fact_for(self.purpose())
    }

    /// The same row with a purpose of the caller's: a raw file that already shows from its
    /// preview says what the plugin adds to it.
    pub fn fact_for(&self, purpose: &str) -> Fact {
        Fact {
            label: FactLabel::Needs,
            value: FactValue::text(format!("{} (to {purpose})", self.package.name())),
        }
    }

    fn purpose(&self) -> &'static str {
        match self.capability {
            Capability::Probe | Capability::Peek => "see what is inside",
            Capability::Thumbnail => "show a picture of it",
            Capability::Decode => "show it",
            Capability::Export => "convert it",
            Capability::Play => "play it",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::Mime;

    #[test]
    fn capabilities_name_their_packages() {
        // name, capability, kind, media type, package
        type Case = (
            &'static str,
            Capability,
            FormatKind,
            Option<&'static str>,
            Option<&'static str>,
        );
        const CASES: &[Case] = &[
            (
                "play video",
                Capability::Play,
                FormatKind::Video,
                None,
                Some("anyview-mpv"),
            ),
            (
                "play audio",
                Capability::Play,
                FormatKind::Audio,
                None,
                Some("anyview-mpv"),
            ),
            (
                "probe video",
                Capability::Probe,
                FormatKind::Video,
                None,
                Some("anyview-ffmpeg"),
            ),
            (
                "export audio",
                Capability::Export,
                FormatKind::Audio,
                None,
                Some("anyview-ffmpeg"),
            ),
            (
                "decode audio has no picture",
                Capability::Decode,
                FormatKind::Audio,
                None,
                None,
            ),
            (
                "a pdf needs no plugin",
                Capability::Probe,
                FormatKind::Pdf,
                None,
                None,
            ),
            (
                "decode heic",
                Capability::Decode,
                FormatKind::Raster,
                Some("image/heic"),
                Some("anyview-heif"),
            ),
            (
                "thumbnail heic",
                Capability::Thumbnail,
                FormatKind::Raster,
                Some("image/heic"),
                Some("anyview-heif"),
            ),
            (
                "decode avif",
                Capability::Decode,
                FormatKind::Raster,
                Some("image/avif"),
                Some("anyview-heif"),
            ),
            (
                "decode raw",
                Capability::Decode,
                FormatKind::Raster,
                Some("image/x-dcraw"),
                Some("anyview-raw"),
            ),
            (
                "thumbnail raw",
                Capability::Thumbnail,
                FormatKind::Raster,
                Some("image/x-dcraw"),
                Some("anyview-raw"),
            ),
            (
                "a png needs no plugin",
                Capability::Decode,
                FormatKind::Raster,
                Some("image/png"),
                None,
            ),
            (
                "a raster of no known type needs none",
                Capability::Decode,
                FormatKind::Raster,
                None,
                None,
            ),
        ];
        for (name, capability, kind, mime, want) in CASES {
            let mime = mime.map(|text| Mime::parse(text).unwrap());
            let subject = Subject {
                kind: *kind,
                mime: mime.as_ref(),
            };
            let got = suggested_package(*capability, &subject).map(Package::name);
            assert_eq!(got, *want, "{name}");
        }
    }

    #[test]
    fn each_package_names_the_tool_its_plugin_runs() {
        // package, helper
        const CASES: &[(Package, Helper)] = &[
            (MPV, Helper::VideoPlayback),
            (FFMPEG, Helper::MediaProbe),
            (HEIF, Helper::HeicDecode),
            (RAW, Helper::RawDecode),
        ];
        for (package, helper) in CASES {
            assert_eq!(package.helper(), *helper, "{}", package.name());
        }
    }

    #[test]
    fn the_fact_names_the_package_and_its_purpose() {
        let missing = MissingPlugin {
            capability: Capability::Probe,
            kind: FormatKind::Video,
            package: FFMPEG,
        };
        let fact = missing.fact();
        assert_eq!(fact.label, FactLabel::Needs);
        assert_eq!(
            fact.value.as_str(),
            "anyview-ffmpeg (to see what is inside)"
        );
    }
}
