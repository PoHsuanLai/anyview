//! The package that would serve a kind when no installed plugin does.

use anyview_core::{Fact, FactLabel, FactValue, FormatKind};
use anyview_plugin_protocol::Capability;

/// The name of a package a person installs with their system's package manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Package(&'static str);

impl Package {
    /// A package by name, for a test to compare against.
    #[cfg(test)]
    pub(crate) const fn from_static(name: &'static str) -> Package {
        Package(name)
    }

    /// The package's name, as every distribution that ships it spells it.
    pub fn name(self) -> &'static str {
        self.0
    }
}

/// Playback, through the user's own mpv.
const MPV: Package = Package("anyview-mpv");
/// Facts, pictures and conversions, through the user's own FFmpeg.
const FFMPEG: Package = Package("anyview-ffmpeg");

/// Which package provides a capability for a kind. A row matches when its capability and its kind
/// are both the ones asked about.
const SUGGESTIONS: &[(Capability, FormatKind, Package)] = &[
    (Capability::Probe, FormatKind::Video, FFMPEG),
    (Capability::Probe, FormatKind::Audio, FFMPEG),
    (Capability::Peek, FormatKind::Video, FFMPEG),
    (Capability::Peek, FormatKind::Audio, FFMPEG),
    (Capability::Thumbnail, FormatKind::Video, FFMPEG),
    (Capability::Thumbnail, FormatKind::Audio, FFMPEG),
    (Capability::Decode, FormatKind::Video, FFMPEG),
    (Capability::Export, FormatKind::Video, FFMPEG),
    (Capability::Export, FormatKind::Audio, FFMPEG),
    (Capability::Play, FormatKind::Video, MPV),
    (Capability::Play, FormatKind::Audio, MPV),
];

/// The package to suggest for `capability` on `kind`, or `None` when no package is known.
pub fn suggested_package(capability: Capability, kind: FormatKind) -> Option<Package> {
    SUGGESTIONS
        .iter()
        .find(|(c, k, _)| *c == capability && *k == kind)
        .map(|(_, _, package)| *package)
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
        let purpose = match self.capability {
            Capability::Probe | Capability::Peek => "see what is inside",
            Capability::Thumbnail => "show a picture of it",
            Capability::Decode => "show it",
            Capability::Export => "convert it",
            Capability::Play => "play it",
        };
        Fact {
            label: FactLabel::Needs,
            value: FactValue::text(format!("{} (to {purpose})", self.package.name())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_capabilities_name_their_packages() {
        // name, capability, kind, package
        const CASES: &[(&str, Capability, FormatKind, Option<&str>)] = &[
            (
                "play video",
                Capability::Play,
                FormatKind::Video,
                Some("anyview-mpv"),
            ),
            (
                "play audio",
                Capability::Play,
                FormatKind::Audio,
                Some("anyview-mpv"),
            ),
            (
                "probe video",
                Capability::Probe,
                FormatKind::Video,
                Some("anyview-ffmpeg"),
            ),
            (
                "export audio",
                Capability::Export,
                FormatKind::Audio,
                Some("anyview-ffmpeg"),
            ),
            (
                "decode audio has no picture",
                Capability::Decode,
                FormatKind::Audio,
                None,
            ),
            (
                "a pdf needs no plugin",
                Capability::Probe,
                FormatKind::Pdf,
                None,
            ),
        ];
        for (name, capability, kind, want) in CASES {
            let got = suggested_package(*capability, *kind).map(Package::name);
            assert_eq!(got, *want, "{name}");
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
