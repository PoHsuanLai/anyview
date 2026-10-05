//! Which plugin serves a kind and capability: the registry built from what discovery found.

use crate::error::PluginError;
use crate::handles::Subject;
use crate::manifest::{Manifest, PluginId};
use crate::missing::{MissingPlugin, suggested_package};
use crate::provision::{Provision, TargetName};
use anyview_plugin_protocol::{Capability, PROTOCOL_VERSION};
use std::cmp::Reverse;

/// Which directory a manifest was found in. The person's own beats the system's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
    /// `$XDG_DATA_DIRS`: installed by the distribution.
    System,
    /// `$XDG_DATA_HOME`: installed by the person.
    User,
}

/// Whether the files a manifest names can be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// Every program is there and runs.
    Ready,
    /// One is not, and this says which and why.
    Unready(PluginError),
}

/// A manifest as discovery found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// What the plugin says of itself.
    pub manifest: Manifest,
    /// Where it was found.
    pub origin: Origin,
    /// Whether its programs are usable.
    pub readiness: Readiness,
}

/// A plugin the viewer can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// What the plugin says of itself.
    pub manifest: Manifest,
    /// Where it was found.
    pub origin: Origin,
}

/// A plugin that was found and cannot be used, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unusable {
    /// Which plugin.
    pub id: PluginId,
    /// Where it was found.
    pub origin: Origin,
    /// Why not.
    pub reason: PluginError,
}

/// What a lookup came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route<'a> {
    /// This plugin serves it.
    Served(&'a Installed),
    /// None does, and this package would.
    Missing(MissingPlugin),
    /// None does and no package is known: nothing to offer.
    Unserved,
}

/// The installed plugins, in the order that decides which serves a request.
///
/// Among manifests with one id, those with an unusable program or a protocol newer than the
/// viewer's are set aside first; of the rest the higher protocol version wins, and at equal
/// versions the person's directory wins over the system's. Plugins with different ids are tried
/// in that same order, then by id, so which one serves a
/// kind never depends on the order the disk listed the files in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plugins {
    installed: Vec<Installed>,
    unusable: Vec<Unusable>,
}

impl Plugins {
    /// No plugins.
    pub fn none() -> Plugins {
        Plugins::default()
    }

    /// The registry for `candidates`, applying the precedence above.
    pub fn resolve(candidates: Vec<Candidate>) -> Plugins {
        let mut usable = Vec::new();
        let mut unusable = Vec::new();
        for candidate in candidates {
            match usability(&candidate) {
                Ok(()) => usable.push(Installed {
                    manifest: candidate.manifest,
                    origin: candidate.origin,
                }),
                Err(reason) => unusable.push(Unusable {
                    id: candidate.manifest.id,
                    origin: candidate.origin,
                    reason,
                }),
            }
        }
        usable.sort_by_key(rank);
        let mut installed: Vec<Installed> = Vec::with_capacity(usable.len());
        for plugin in usable {
            if installed
                .iter()
                .all(|kept| kept.manifest.id != plugin.manifest.id)
            {
                installed.push(plugin);
            }
        }
        unusable.sort_by(|a, b| (&a.id, a.origin).cmp(&(&b.id, b.origin)));
        Plugins {
            installed,
            unusable,
        }
    }

    /// The usable plugins, in serving order.
    pub fn installed(&self) -> &[Installed] {
        &self.installed
    }

    /// The plugins that were found and cannot be used.
    pub fn unusable(&self) -> &[Unusable] {
        &self.unusable
    }

    /// The plugin that serves `capability` for `subject`: one that lists the subject's MIME type
    /// beats one that lists only its kind; ties go in serving order.
    pub fn serving(&self, capability: Capability, subject: &Subject<'_>) -> Option<&Installed> {
        let serves = |plugin: &&Installed| {
            plugin
                .manifest
                .provision(capability)
                .is_some_and(|provision| provision.handles().handles(subject))
        };
        let by_mime = |plugin: &&Installed| {
            plugin
                .manifest
                .provision(capability)
                .is_some_and(|provision| provision.handles().names_mime(subject))
        };
        let mut serving = self.installed.iter().filter(serves).peekable();
        let first = serving.peek().copied();
        serving.find(by_mime).or(first)
    }

    /// What a request for `capability` on `subject` comes to.
    pub fn route(&self, capability: Capability, subject: &Subject<'_>) -> Route<'_> {
        if let Some(plugin) = self.serving(capability, subject) {
            return Route::Served(plugin);
        }
        match suggested_package(capability, subject) {
            Some(package) => Route::Missing(MissingPlugin {
                capability,
                kind: subject.kind,
                package,
            }),
            None => Route::Unserved,
        }
    }

    /// The targets the plugins that export `subject` can write, each with the plugin that
    /// writes it: what an export sheet offers. A target two plugins write is listed for the
    /// first of them.
    pub fn export_targets(&self, subject: &Subject<'_>) -> Vec<(&Installed, &TargetName)> {
        let mut offered: Vec<(&Installed, &TargetName)> = Vec::new();
        for plugin in &self.installed {
            let Some(Provision::Export(export)) = plugin.manifest.provision(Capability::Export)
            else {
                continue;
            };
            if !export.handles.handles(subject) {
                continue;
            }
            for target in &export.targets {
                if offered.iter().all(|(_, seen)| *seen != target) {
                    offered.push((plugin, target));
                }
            }
        }
        offered
    }
}

fn usability(candidate: &Candidate) -> Result<(), PluginError> {
    let protocol = candidate.manifest.protocol;
    if protocol > PROTOCOL_VERSION {
        return Err(PluginError::ProtocolUnsupported {
            protocol,
            supported: PROTOCOL_VERSION,
        });
    }
    match &candidate.readiness {
        Readiness::Ready => Ok(()),
        Readiness::Unready(reason) => Err(reason.clone()),
    }
}

/// The serving order: the person's before the system's, newer protocol first, then by id.
fn rank(plugin: &Installed) -> (Reverse<u32>, Reverse<Origin>, PluginId) {
    (
        Reverse(plugin.manifest.protocol),
        Reverse(plugin.origin),
        plugin.manifest.id.clone(),
    )
}

#[cfg(test)]
mod tests;
