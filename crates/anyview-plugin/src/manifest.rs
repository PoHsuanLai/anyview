//! The manifest: the TOML a plugin installs at `<data dir>/anyview/plugins/<id>.toml`, parsed
//! and checked once so everything past it holds a value that cannot be malformed.

use crate::error::PluginError;
use crate::handles::Handles;
use crate::provision::{ExportProvision, PlayProvision, Player, Provision, TargetName};
use anyview_core::{FormatKind, Mime};
use anyview_plugin_protocol::Capability;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// A plugin's id: lower-case letters, digits, `-` and `_`, up to 64 of them. It names the
/// manifest file and decides which manifest overrides which.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PluginId(String);

impl PluginId {
    /// `text` as an id, or why it is not one.
    pub fn parse(text: &str) -> Result<PluginId, PluginError> {
        let fits = !text.is_empty()
            && text.len() <= 64
            && text
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if fits {
            Ok(PluginId(text.to_owned()))
        } else {
            Err(PluginError::IdInvalid {
                id: text.to_owned(),
            })
        }
    }

    /// The id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The executable that speaks the protocol, and the arguments it is started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// An absolute path.
    pub path: PathBuf,
    /// Arguments placed before anything the host adds (nothing, today).
    pub args: Vec<String>,
}

/// A plugin as it describes itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Names the file and the plugin.
    pub id: PluginId,
    /// What a person calls it.
    pub name: String,
    /// The newest protocol version it speaks.
    pub protocol: u32,
    /// What speaks the protocol; absent for a plugin that only provides `play`.
    pub program: Option<Program>,
    /// What it provides, one entry for each capability.
    pub provides: Vec<Provision>,
}

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    name: String,
    protocol: u32,
    program: Option<RawProgram>,
    #[serde(default)]
    provides: Vec<RawProvide>,
}

#[derive(Deserialize)]
struct RawProgram {
    path: PathBuf,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Deserialize)]
struct RawProvide {
    capability: Capability,
    #[serde(default)]
    kinds: Vec<FormatKind>,
    #[serde(default)]
    mimes: Vec<Mime>,
    #[serde(default)]
    targets: Vec<String>,
    mpv: Option<PathBuf>,
    cplugin: Option<PathBuf>,
}

impl Manifest {
    /// Parses and checks the text of a manifest.
    pub fn parse(text: &str) -> Result<Manifest, PluginError> {
        let raw: RawManifest = toml::from_str(text).map_err(|error| PluginError::Syntax {
            reason: error.message().to_owned(),
        })?;
        let id = PluginId::parse(&raw.id)?;
        if raw.name.trim().is_empty() {
            return Err(PluginError::NameEmpty);
        }
        if raw.protocol == 0 {
            return Err(PluginError::ProtocolZero);
        }
        let program = raw.program.map(program).transpose()?;
        let provides = provisions(raw.provides)?;
        if provides.iter().any(Provision::needs_program) && program.is_none() {
            let capability = provides
                .iter()
                .find(|provision| provision.needs_program())
                .map_or(Capability::Probe, Provision::capability);
            return Err(PluginError::ProgramMissing { capability });
        }
        Ok(Manifest {
            id,
            name: raw.name,
            protocol: raw.protocol,
            program,
            provides,
        })
    }

    /// Checks that the file the manifest was read from is called `<id>.toml`.
    pub fn check_file_name(&self, file: &Path) -> Result<(), PluginError> {
        let stem = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem == self.id.as_str() {
            Ok(())
        } else {
            Err(PluginError::IdFileMismatch {
                id: self.id.as_str().to_owned(),
                file: stem.to_owned(),
            })
        }
    }

    /// The provision of `capability`, if the plugin has one.
    pub fn provision(&self, capability: Capability) -> Option<&Provision> {
        self.provides
            .iter()
            .find(|provision| provision.capability() == capability)
    }

    /// Every path the manifest names that must exist and be executable or loadable: the
    /// program, the player's `mpv` and its C plugin.
    pub fn paths(&self) -> Vec<&Path> {
        let program = self.program.iter().map(|program| program.path.as_path());
        let players = self
            .provides
            .iter()
            .filter_map(|provision| match provision {
                Provision::Play(play) => {
                    Some([play.player.mpv.as_path(), play.player.cplugin.as_path()])
                }
                Provision::Probe(_)
                | Provision::Peek(_)
                | Provision::Thumbnail(_)
                | Provision::Decode(_)
                | Provision::Export(_) => None,
            });
        program.chain(players.flatten()).collect()
    }
}

fn absolute(path: PathBuf) -> Result<PathBuf, PluginError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(PluginError::PathNotAbsolute { path })
    }
}

fn program(raw: RawProgram) -> Result<Program, PluginError> {
    Ok(Program {
        path: absolute(raw.path)?,
        args: raw.args,
    })
}

fn provisions(raw: Vec<RawProvide>) -> Result<Vec<Provision>, PluginError> {
    if raw.is_empty() {
        return Err(PluginError::NothingProvided);
    }
    let mut provides: Vec<Provision> = Vec::with_capacity(raw.len());
    for entry in raw {
        let provision = provision(entry)?;
        let capability = provision.capability();
        if provides.iter().any(|seen| seen.capability() == capability) {
            return Err(PluginError::CapabilityRepeated { capability });
        }
        provides.push(provision);
    }
    Ok(provides)
}

fn provision(raw: RawProvide) -> Result<Provision, PluginError> {
    let capability = raw.capability;
    let player = (raw.mpv.is_some() || raw.cplugin.is_some()) && capability != Capability::Play;
    if player {
        return Err(PluginError::PlayerMisplaced { capability });
    }
    let targets_misplaced = !raw.targets.is_empty() && capability != Capability::Export;
    if targets_misplaced {
        return Err(PluginError::TargetsMisplaced { capability });
    }
    let handles = Handles::new(capability, raw.kinds, raw.mimes)?;
    match capability {
        Capability::Probe => Ok(Provision::Probe(handles)),
        Capability::Peek => Ok(Provision::Peek(handles)),
        Capability::Thumbnail => Ok(Provision::Thumbnail(handles)),
        Capability::Decode => Ok(Provision::Decode(handles)),
        Capability::Export => {
            let targets = raw
                .targets
                .iter()
                .map(|target| TargetName::parse(target))
                .collect::<Result<Vec<_>, _>>()?;
            if targets.is_empty() {
                return Err(PluginError::TargetsMissing);
            }
            Ok(Provision::Export(ExportProvision { handles, targets }))
        }
        Capability::Play => match (raw.mpv, raw.cplugin) {
            (Some(mpv), Some(cplugin)) => Ok(Provision::Play(PlayProvision {
                handles,
                player: Player {
                    mpv: absolute(mpv)?,
                    cplugin: absolute(cplugin)?,
                },
            })),
            (None, Some(_)) | (Some(_), None) | (None, None) => Err(PluginError::PlayerIncomplete),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
