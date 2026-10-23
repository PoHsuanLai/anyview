//! Finding the plugins: the Extensions under `quire/extensions` in the data directories (and, for
//! one more release, the flat manifests under `anyview/plugins`), whether the programs they name
//! are there, and which are turned off. The scan is quire's (`ds-extensions`), the entries are
//! this viewer's (`anyview-plugin`), the ranking is bayonet's; this names the roots.

use crate::env::Env;
use anyview_plugin::{OffPlugin, PluginError, Plugins, Provision, shipped_extension_id};
use anyview_plugin_protocol::PROTOCOL_VERSION;
use bayonet::manifest::{ManifestError, PluginId, Raw, RawCapability};
use bayonet::registry::{Candidate, Registry};
use bayonet::{Reparsed, reparse};
use ds_extensions::{ExtensionId, ExtensionsFile, Search};
use ds_settings::{AppName, ConfigRoot, Store};
use std::convert::Infallible;
use std::path::{Path, PathBuf};

/// The file an Extension's folder holds its manifest in (`ds-extensions` keeps the name to itself;
/// it is only used to say which file a refusal is about).
const MANIFEST_FILE: &str = "extension.toml";

/// A manifest file that is not a plugin.
pub type Rejected = bayonet::Rejected<Provision>;

/// What was found: the registry, and each file that could not become a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    /// The plugins that can be used, the ones that were found and cannot be, and the ones that
    /// are turned off.
    pub plugins: Plugins,
    /// Manifest files that were unreadable, malformed or misnamed.
    pub rejected: Vec<Rejected>,
}

/// Reads every Extension manifest in `$XDG_DATA_HOME/quire/extensions` (the person's) and in each
/// of `$XDG_DATA_DIRS` (the system's, most important first), as `env` names them, and the ones
/// still in the old place for one release. A file that is not a usable manifest is listed in
/// `rejected` and skipped; nothing here fails.
#[must_use]
pub fn discover(env: &Env) -> Discovery {
    discover_in(env, &env.dirs.data)
}

/// [`discover`] with `folder` as the person's data directory, whose `quire/extensions` is read
/// first: a host that keeps its plugins elsewhere (a development tree, a portable install) names
/// the folder, and `env`'s system directories still follow.
///
/// A plugin is usable when its program is there and its protocol is one this viewer speaks; those
/// are set aside before anything is ranked, so a person's copy that cannot run never hides a
/// system one that can. Of the usable copies of one id the higher protocol wins, then the
/// person's over the system's, then the newer version. An Extension turned off in
/// `quire/extensions.toml` (in `env`'s config directory) serves nothing and is listed in
/// [`Plugins::off`].
#[must_use]
pub fn discover_in(env: &Env, folder: &Path) -> Discovery {
    let off = off_ids(env);
    let found = Search::new(folder, &env.dirs.data_dirs).scan(&off);
    let mut rejected: Vec<Rejected> = found
        .rejected
        .into_iter()
        .map(|refused| Rejected::new(refused.file, retyped(refused.error)))
        .collect();

    let mut on = typed(found.candidates, &mut rejected);
    let mut turned_off = Vec::new();
    for apart in found.off {
        turned_off.extend(typed(apart.candidates, &mut rejected));
    }

    // The old place, after the new one: at one root and one id the new location's copy comes
    // first in the list, and bayonet's stable ranking keeps it there.
    let (old, old_rejected) = legacy(env, folder);
    rejected.extend(old_rejected);
    for candidate in old {
        let is_off = off.iter().any(|id| id.0 == candidate.manifest.id.as_str());
        let side = if is_off { &mut turned_off } else { &mut on };
        side.push(candidate);
    }

    let registry = Registry::resolve(on, PROTOCOL_VERSION);
    let off_plugins = Registry::resolve(turned_off, PROTOCOL_VERSION)
        .installed()
        .iter()
        .map(|plugin| {
            OffPlugin::new(
                plugin.manifest.id.as_str(),
                plugin.manifest.name.clone(),
                plugin.manifest.provides.clone(),
            )
        })
        .collect();
    Discovery {
        plugins: Plugins::from(registry).with_off(off_plugins),
        rejected,
    }
}

/// The ids turned off in `quire/extensions.toml` in `env`'s config directory. A file that is not
/// there, or cannot be read, turns nothing off.
fn off_ids(env: &Env) -> Vec<ExtensionId> {
    let store = Store::new(ConfigRoot::Scratch(env.dirs.config.clone()), AppName::QUIRE);
    store.load::<ExtensionsFile>().value.off_ids()
}

/// `scanned`, the Extensions a scan kept with their entries raw, as this viewer's candidates:
/// the entries of its six words, read and checked. An Extension with none of them (a look, file
/// icons) is not a plugin and is left out; one whose entry is refused goes to `rejected`.
fn typed(scanned: Vec<Candidate<Raw>>, rejected: &mut Vec<Rejected>) -> Vec<Candidate<Provision>> {
    let mut candidates = Vec::new();
    for raw in scanned {
        let file = manifest_file(&raw);
        match reparse::<Provision>(raw) {
            Ok(Reparsed {
                candidate: Some(candidate),
                ..
            }) => candidates.push(candidate),
            Ok(Reparsed {
                candidate: None, ..
            }) => {}
            Err(error) => rejected.push(Rejected::new(file, error)),
        }
    }
    candidates
}

/// The manifest `raw` was read from, for a refusal to name.
fn manifest_file(raw: &Candidate<Raw>) -> PathBuf {
    match raw
        .manifest
        .provides
        .first()
        .and_then(|entry| entry.folder())
    {
        Some(folder) => folder.join(MANIFEST_FILE),
        None => PathBuf::from(raw.manifest.id.as_str()),
    }
}

/// A refusal of a raw scan as this viewer's own type. The scan reads no entries, so only the
/// reasons about the file and the envelope can occur; any other is said in words.
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "bayonet's ManifestError is non_exhaustive, so a foreign match needs a fallback"
)]
fn retyped(error: ManifestError<RawCapability, Infallible>) -> PluginError {
    match error {
        ManifestError::Unreadable { kind } => PluginError::Unreadable { kind },
        ManifestError::Syntax { reason } => PluginError::Syntax { reason },
        ManifestError::IdInvalid { id } => PluginError::IdInvalid { id },
        ManifestError::IdFolderMismatch { id, folder } => {
            PluginError::IdFolderMismatch { id, folder }
        }
        ManifestError::NameEmpty => PluginError::NameEmpty,
        ManifestError::VersionInvalid { version } => PluginError::VersionInvalid { version },
        ManifestError::PathEscapes { path } => PluginError::PathEscapes { path },
        ManifestError::ProtocolZero => PluginError::ProtocolZero,
        ManifestError::NothingProvided => PluginError::NothingProvided,
        other => PluginError::Syntax {
            reason: other.to_string(),
        },
    }
}

/// THE OLD PLACE, read for one release and then deleted with this function: the flat manifests
/// `<data dir>/anyview/plugins/<id>.toml` that earlier installs wrote. The four plugins this
/// repository ships were `ffmpeg`, `heif`, `raw` and `mpv`; each is given the Extension id it has
/// now before anything is ranked, so an old install and a new one are two copies of one id
/// (and the new location's wins at the same root) instead of two plugins, of which the old id
/// would serve first because ids sort. Any other id is a plugin somebody else wrote and is read
/// as it stands.
fn legacy(env: &Env, folder: &Path) -> (Vec<Candidate<Provision>>, Vec<Rejected>) {
    let scan = bayonet::scan::<Provision>(&bayonet::Search::new(
        "anyview",
        folder,
        &env.dirs.data_dirs,
        PROTOCOL_VERSION,
    ));
    let candidates = scan
        .candidates
        .into_iter()
        .map(|mut candidate| {
            let renamed = shipped_extension_id(candidate.manifest.id.as_str())
                .and_then(|id| PluginId::parse_dotted(id).ok());
            if let Some(id) = renamed {
                candidate.manifest.id = id;
            }
            candidate
        })
        .collect();
    (candidates, scan.rejected)
}
