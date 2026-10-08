//! One capability a manifest provides, with what it handles.

use crate::error::ProvisionFault;
use crate::handles::Handles;
use anyview_core::{FormatKind, Mime};
use anyview_plugin_protocol::Capability;
use bayonet::manifest::{Entry, PathRole, Provides, Refusal, absolute};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The name of an export target (`mp3`, `flac`, `webm`) as the protocol's `export` request
/// spells it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetName(String);

impl TargetName {
    /// `text` as a target name, or why it is not one.
    pub fn parse(text: &str) -> Result<TargetName, ProvisionFault> {
        let fits = !text.is_empty()
            && text.len() <= 32
            && text
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c));
        if fits {
            Ok(TargetName(text.to_owned()))
        } else {
            Err(ProvisionFault::TargetInvalid {
                target: text.to_owned(),
            })
        }
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An `export`: what it reads and what it can write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportProvision {
    /// What it reads.
    pub handles: Handles,
    /// What it writes. Never empty.
    pub targets: Vec<TargetName>,
}

/// What the player that `mpv-wgpu` starts needs: the stock `mpv` and the C plugin it loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// The `mpv` executable.
    pub mpv: PathBuf,
    /// The shared object loaded into it with `--script`.
    pub cplugin: PathBuf,
}

/// A `play`: what it plays and the player's paths. The launch is `mpv-wgpu`'s; this crate only
/// carries the paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayProvision {
    /// What it plays.
    pub handles: Handles,
    /// How to start it.
    pub player: Player,
}

/// One capability a plugin provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provision {
    /// Facts about a file.
    Probe(Handles),
    /// The launcher pane's facts and picture.
    Peek(Handles),
    /// A small picture.
    Thumbnail(Handles),
    /// A picture within a pixel budget.
    Decode(Handles),
    /// Writing another format.
    Export(ExportProvision),
    /// Playing a recording.
    Play(PlayProvision),
}

impl Provision {
    /// Which capability this is.
    pub fn capability(&self) -> Capability {
        match self {
            Provision::Probe(_) => Capability::Probe,
            Provision::Peek(_) => Capability::Peek,
            Provision::Thumbnail(_) => Capability::Thumbnail,
            Provision::Decode(_) => Capability::Decode,
            Provision::Export(_) => Capability::Export,
            Provision::Play(_) => Capability::Play,
        }
    }

    /// What it handles.
    pub fn handles(&self) -> &Handles {
        match self {
            Provision::Probe(handles)
            | Provision::Peek(handles)
            | Provision::Thumbnail(handles)
            | Provision::Decode(handles) => handles,
            Provision::Export(export) => &export.handles,
            Provision::Play(play) => &play.handles,
        }
    }

    /// Whether the plugin's `program` speaks it over the protocol (everything but `play`).
    pub fn needs_program(&self) -> bool {
        match self {
            Provision::Probe(_)
            | Provision::Peek(_)
            | Provision::Thumbnail(_)
            | Provision::Decode(_)
            | Provision::Export(_) => true,
            Provision::Play(_) => false,
        }
    }
}

/// The keys of one `[[provides]]` table, as written.
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

impl Provides for Provision {
    type Capability = Capability;
    type Fault = ProvisionFault;

    fn parse(entry: Entry) -> Result<Provision, Refusal<ProvisionFault>> {
        let raw: RawProvide = entry.read()?;
        let capability = raw.capability;
        let player = (raw.mpv.is_some() || raw.cplugin.is_some()) && capability != Capability::Play;
        if player {
            return Err(Refusal::Fault(ProvisionFault::PlayerMisplaced {
                capability,
            }));
        }
        let targets_misplaced = !raw.targets.is_empty() && capability != Capability::Export;
        if targets_misplaced {
            return Err(Refusal::Fault(ProvisionFault::TargetsMisplaced {
                capability,
            }));
        }
        let handles = Handles::new(capability, raw.kinds, raw.mimes).map_err(Refusal::Fault)?;
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
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(Refusal::Fault)?;
                if targets.is_empty() {
                    return Err(Refusal::Fault(ProvisionFault::TargetsMissing));
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
                (None, Some(_)) | (Some(_), None) | (None, None) => {
                    Err(Refusal::Fault(ProvisionFault::PlayerIncomplete))
                }
            },
        }
    }

    fn capability(&self) -> Capability {
        Provision::capability(self)
    }

    fn needs_program(&self) -> bool {
        Provision::needs_program(self)
    }

    fn paths(&self) -> Vec<(&Path, PathRole)> {
        match self {
            Provision::Play(play) => vec![
                (play.player.mpv.as_path(), PathRole::Executable),
                (play.player.cplugin.as_path(), PathRole::Library),
            ],
            Provision::Probe(_)
            | Provision::Peek(_)
            | Provision::Thumbnail(_)
            | Provision::Decode(_)
            | Provision::Export(_) => Vec::new(),
        }
    }
}
