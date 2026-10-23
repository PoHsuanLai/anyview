//! Which plugin serves a kind and capability: the registry built from what discovery found. The
//! ranking is bayonet's; what "serves a kind" means is the viewer's.

use crate::extension_id::package_extension_id;
use crate::handles::Subject;
use crate::missing::{MissingPlugin, Package, suggested_package};
use crate::off::OffPlugin;
use crate::provision::{Provision, TargetName};
use anyview_plugin_protocol::{Capability, PROTOCOL_VERSION};
use bayonet::registry::{Fit, Registry};

pub use bayonet::registry::Origin;

/// Whether the files a manifest names can be used.
pub type Readiness = bayonet::registry::Readiness<Provision>;

/// A manifest as discovery found it.
pub type Candidate = bayonet::registry::Candidate<Provision>;

/// A plugin the viewer can use.
pub type Installed = bayonet::registry::Installed<Provision>;

/// A plugin that was found and cannot be used, with the reason.
pub type Unusable = bayonet::registry::Unusable<Provision>;

/// What a lookup came to. A plugin that is turned off is not missing: it is named so the viewer can
/// offer to turn it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route<'a> {
    /// This plugin serves it.
    Served(&'a Installed),
    /// None that is on does, and this one would if it were turned on.
    Off(&'a OffPlugin),
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
/// in that same order, then by id, so which one serves a kind never depends on the order the disk
/// listed the files in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plugins {
    registry: Registry<Provision>,
    off: Vec<OffPlugin>,
}

impl From<Registry<Provision>> for Plugins {
    fn from(registry: Registry<Provision>) -> Plugins {
        Plugins {
            registry,
            off: Vec::new(),
        }
    }
}

impl Plugins {
    /// No plugins.
    #[must_use]
    pub fn none() -> Plugins {
        Plugins::default()
    }

    /// The registry for `candidates`, applying the precedence above.
    #[must_use]
    pub fn resolve(candidates: Vec<Candidate>) -> Plugins {
        Plugins::from(Registry::resolve(candidates, PROTOCOL_VERSION))
    }

    /// These plugins with `off` as the ones that are installed and turned off: they serve
    /// nothing, and [`Plugins::route`] names one when it would have been the answer.
    #[must_use]
    pub fn with_off(self, off: Vec<OffPlugin>) -> Plugins {
        Plugins { off, ..self }
    }

    /// The plugins that are installed and turned off.
    #[must_use]
    pub fn off(&self) -> &[OffPlugin] {
        &self.off
    }

    /// The usable plugins, in serving order.
    #[must_use]
    pub fn installed(&self) -> &[Installed] {
        self.registry.installed()
    }

    /// The plugins that were found and cannot be used.
    #[must_use]
    pub fn unusable(&self) -> &[Unusable] {
        self.registry.unusable()
    }

    /// The plugin that serves `capability` for `subject`: one that lists the subject's MIME type
    /// beats one that lists only its kind; ties go in serving order.
    #[must_use]
    pub fn serving(&self, capability: Capability, subject: &Subject<'_>) -> Option<&Installed> {
        self.registry.serving(capability, |provision| {
            let handles = provision.handles();
            if handles.names_mime(subject) {
                Fit::Exact
            } else if handles.handles(subject) {
                Fit::Broad
            } else {
                Fit::Miss
            }
        })
    }

    /// What a request for `capability` on `subject` comes to.
    #[must_use]
    pub fn route(&self, capability: Capability, subject: &Subject<'_>) -> Route<'_> {
        if let Some(plugin) = self.serving(capability, subject) {
            return Route::Served(plugin);
        }
        if let Some(off) = self
            .off
            .iter()
            .find(|plugin| plugin.serves(capability, subject))
        {
            return Route::Off(off);
        }
        match self.missing_for(capability, subject) {
            Some(missing) => Route::Missing(missing),
            None => Route::Unserved,
        }
    }

    /// The package that provides `capability` for `subject`, whether or not it is installed: what
    /// an engine that knows only [`MissingPlugin`] is handed for a [`Route::Off`] too, so that
    /// [`Plugins::off_package`] can tell the two apart where the words are chosen.
    #[must_use]
    pub fn missing_for(
        &self,
        capability: Capability,
        subject: &Subject<'_>,
    ) -> Option<MissingPlugin> {
        suggested_package(capability, subject).map(|package| MissingPlugin {
            capability,
            kind: subject.kind,
            package,
        })
    }

    /// The installed plugin that `package` names, when it is turned off.
    #[must_use]
    pub fn off_package(&self, package: Package) -> Option<&OffPlugin> {
        let id = package_extension_id(package)?;
        self.off.iter().find(|plugin| plugin.id() == id)
    }

    /// Whether the plugin `package` names is installed and cannot run only because the program of
    /// the system's that its manifest names is absent: the tool is what is missing, not the
    /// plugin, and installing the tool is what makes the plugin usable.
    #[must_use]
    pub fn tool_absent(&self, package: Package) -> bool {
        let Some(tool) = package.tool else {
            return false;
        };
        let Some(id) = package_extension_id(package) else {
            return false;
        };
        self.registry.tool_absent(id, tool)
    }

    /// The targets the plugins that export `subject` can write, each with the plugin that
    /// writes it: what an export sheet offers. A target two plugins write is listed for the
    /// first of them.
    #[must_use]
    pub fn export_targets(&self, subject: &Subject<'_>) -> Vec<(&Installed, &TargetName)> {
        let mut offered: Vec<(&Installed, &TargetName)> = Vec::new();
        for plugin in self.installed() {
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

#[cfg(test)]
mod tests;
