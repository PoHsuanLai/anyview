//! The manifest: the TOML a plugin installs at `<data dir>/anyview/plugins/<id>.toml`, parsed
//! and checked once so everything past it holds a value that cannot be malformed. The envelope
//! (id, name, protocol, program) is bayonet's; the viewer's own part is [`Provision`], which
//! reads each `[[provides]]` table.

use crate::provision::Provision;

pub use bayonet::manifest::{PathRole, PluginId, Program};

/// A plugin as it describes itself.
pub type Manifest = bayonet::manifest::Manifest<Provision>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PluginError;
    use crate::provision::TargetName;
    use anyview_plugin_protocol::Capability;
    use std::path::Path;

    const FULL: &str = r#"
id = "ffmpeg"
name = "FFmpeg"
protocol = 1

[program]
path = "/usr/libexec/anyview/anyview-ffmpeg"
args = ["--serve"]

[[provides]]
capability = "probe"
kinds = ["video", "audio"]

[[provides]]
capability = "thumbnail"
kinds = ["video"]
mimes = ["video/x-extra"]

[[provides]]
capability = "export"
kinds = ["audio"]
targets = ["mp3", "flac"]

[[provides]]
capability = "play"
kinds = ["video", "audio"]
mpv = "/usr/bin/mpv"
cplugin = "/usr/lib/anyview/mpv-wgpu-cplugin.so"
"#;

    #[test]
    fn a_full_manifest_parses_to_typed_values() {
        let manifest = Manifest::parse(FULL).unwrap();
        assert_eq!(manifest.id.as_str(), "ffmpeg");
        assert_eq!(manifest.protocol, 1);
        assert_eq!(
            manifest.program.as_ref().map(|p| p.args.clone()),
            Some(vec!["--serve".to_owned()])
        );
        let capabilities: Vec<_> = manifest
            .provides
            .iter()
            .map(Provision::capability)
            .collect();
        assert_eq!(
            capabilities,
            [
                Capability::Probe,
                Capability::Thumbnail,
                Capability::Export,
                Capability::Play
            ]
        );
        let Some(Provision::Export(export)) = manifest.provision(Capability::Export) else {
            panic!("an export");
        };
        let targets: Vec<_> = export.targets.iter().map(TargetName::as_str).collect();
        assert_eq!(targets, ["mp3", "flac"]);
        assert_eq!(manifest.paths().len(), 3);
    }

    #[test]
    fn a_play_only_manifest_needs_no_program() {
        let text = r#"
id = "mpv"
name = "mpv"
protocol = 1
[[provides]]
capability = "play"
kinds = ["video"]
mpv = "/usr/bin/mpv"
cplugin = "/c.so"
"#;
        assert!(Manifest::parse(text).unwrap().program.is_none());
    }

    /// A manifest with one provide entry written as `entry` and the program table.
    fn with(entry: &str) -> String {
        format!(
            "id = \"p\"\nname = \"P\"\nprotocol = 1\n[program]\npath = \"/bin/p\"\n[[provides]]\n{entry}\n"
        )
    }

    #[test]
    fn a_bad_manifest_is_a_typed_error() {
        // name, text, the error's variant name
        let cases: Vec<(&str, String, &str)> = vec![
            ("not toml", "id = ".to_owned(), "Syntax"),
            (
                "unknown capability",
                with("capability = \"dance\"\nkinds = [\"video\"]"),
                "Syntax",
            ),
            (
                "unknown kind",
                with("capability = \"probe\"\nkinds = [\"hologram\"]"),
                "Syntax",
            ),
            (
                "bad mime",
                with("capability = \"probe\"\nmimes = [\"nonsense\"]"),
                "Syntax",
            ),
            (
                "bad id",
                with("capability = \"probe\"\nkinds = [\"video\"]").replace("\"p\"", "\"P q\""),
                "IdInvalid",
            ),
            (
                "protocol zero",
                with("capability = \"probe\"\nkinds = [\"video\"]")
                    .replace("protocol = 1", "protocol = 0"),
                "ProtocolZero",
            ),
            (
                "handles nothing",
                with("capability = \"probe\""),
                "HandlesNothing",
            ),
            (
                "export without targets",
                with("capability = \"export\"\nkinds = [\"video\"]"),
                "TargetsMissing",
            ),
            (
                "targets on probe",
                with("capability = \"probe\"\nkinds = [\"video\"]\ntargets = [\"mp3\"]"),
                "TargetsMisplaced",
            ),
            (
                "bad target",
                with("capability = \"export\"\nkinds = [\"video\"]\ntargets = [\"MP 3\"]"),
                "TargetInvalid",
            ),
            (
                "play without cplugin",
                with("capability = \"play\"\nkinds = [\"video\"]\nmpv = \"/usr/bin/mpv\""),
                "PlayerIncomplete",
            ),
            (
                "player paths on probe",
                with("capability = \"probe\"\nkinds = [\"video\"]\nmpv = \"/usr/bin/mpv\""),
                "PlayerMisplaced",
            ),
            (
                "relative program",
                with("capability = \"probe\"\nkinds = [\"video\"]")
                    .replace("/bin/p", "bin/p"),
                "PathNotAbsolute",
            ),
            (
                "protocol capability with no program",
                "id = \"p\"\nname = \"P\"\nprotocol = 1\n[[provides]]\ncapability = \"probe\"\nkinds = [\"video\"]\n"
                    .to_owned(),
                "ProgramMissing",
            ),
            (
                "nothing provided",
                "id = \"p\"\nname = \"P\"\nprotocol = 1\n".to_owned(),
                "NothingProvided",
            ),
        ];
        for (name, text, want) in cases {
            let error = Manifest::parse(&text).expect_err(name);
            let got = format!("{error:?}");
            assert!(got.starts_with(want), "{name}: wanted {want}, got {got}");
        }
    }

    #[test]
    fn a_capability_listed_twice_is_refused() {
        let text = format!(
            "{}[[provides]]\ncapability = \"probe\"\nkinds = [\"audio\"]\n",
            with("capability = \"probe\"\nkinds = [\"video\"]")
        );
        assert_eq!(
            Manifest::parse(&text).unwrap_err(),
            PluginError::CapabilityRepeated {
                capability: Capability::Probe
            }
        );
    }

    #[test]
    fn the_file_must_be_named_for_the_id() {
        let manifest = Manifest::parse(FULL).unwrap();
        assert!(
            manifest
                .check_file_name(Path::new("/x/ffmpeg.toml"))
                .is_ok()
        );
        assert!(
            manifest
                .check_file_name(Path::new("/x/other.toml"))
                .is_err()
        );
    }
}
