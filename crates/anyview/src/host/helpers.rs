//! The tools the plugins run, and what to do when the system lacks one. quire's `ds-helpers`
//! knows how to look for a tool, ask PackageKit to install it (which asks the person for the
//! password itself) and hear that it appeared; this is the viewer's end of it: which tool each
//! plugin needs, the words for the sheet, the install as a task, and telling every window that a
//! tool is there so the file that waited for it opens again.

use super::plugin_registry::PluginRegistry;
use anyview_core::{Fact, Helper};
use anyview_ui::{HelperEnd, HelperSource, HelperWords, Need};
use ds::prelude::Word;
use ds_helpers::{
    Capability, Catalog, Entry, Environment, Helpers, Installer, Missing, Outcome, Presence,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// What the sheet calls the program.
const APP: &str = "Anyview";

/// What a window wants to hear when a tool appears.
type Told = Arc<dyn Fn(Helper) + Send + Sync>;

/// The windows listening for a tool to appear, by the number each was given.
#[derive(Default)]
struct Listeners {
    next: u64,
    told: BTreeMap<u64, Told>,
}

/// The viewer's missing tools: the catalog of what each plugin runs, the install, and the windows
/// to tell.
pub struct HelperHost {
    helpers: Helpers,
    /// Whether the system can install a package at all (PackageKit answers).
    installable: bool,
    registry: PluginRegistry,
    listeners: Mutex<Listeners>,
}

impl std::fmt::Debug for HelperHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelperHost").finish_non_exhaustive()
    }
}

/// The capability a tool is declared under in the helpers file.
fn capability_of(helper: Helper) -> Option<Capability> {
    Capability::new(helper.slug()).ok()
}

/// The tool a capability of the helpers file names, when it is one of ours.
fn helper_of(capability: &Capability) -> Option<Helper> {
    Helper::ALL
        .iter()
        .copied()
        .find(|helper| helper.slug() == capability.as_str())
}

/// The package to look for in a software centre: what the request named, else the program, which a
/// software centre also finds.
fn looked_for(missing: &Missing) -> String {
    match (&missing.package, &missing.program) {
        (Some(package), _) => package.to_string(),
        (None, Some(program)) => program.to_string(),
        (None, None) => missing.capability.to_string(),
    }
}

/// The program a package must provide: what the request named, else the package.
fn needed_on_path(missing: &Missing) -> String {
    match (&missing.program, &missing.package) {
        (Some(program), _) => program.to_string(),
        (None, Some(package)) => package.to_string(),
        (None, None) => missing.capability.to_string(),
    }
}

impl HelperHost {
    /// A host over `catalog`, looking for tools in `environment` and installing through
    /// `installer`. A tool that appears makes `registry` read the plugins again.
    pub fn new(
        catalog: Catalog,
        environment: Environment,
        installer: Installer,
        registry: PluginRegistry,
    ) -> HelperHost {
        HelperHost {
            helpers: Helpers::new(catalog, environment, installer),
            installable: true,
            registry,
            listeners: Mutex::default(),
        }
    }

    /// The same host, offering to install a tool only where `installable`: the system has a way
    /// to install one (quire's `Helpers` capability, PackageKit). Without it a missing tool stays
    /// a row that names it, with no Install... beside it.
    pub fn installing_where(self, installable: bool) -> HelperHost {
        HelperHost {
            installable,
            ..self
        }
    }

    /// Whether the viewer can offer to install `helper`: the helpers file declares it, so the
    /// viewer can word the question, and the system can install it.
    pub fn offers(&self, helper: Helper) -> bool {
        self.installable && self.entry(helper).is_some()
    }

    /// The `Needs` row `fact`, with `helper` to install when the file declares it.
    pub fn need(&self, fact: Fact, helper: Helper) -> Need {
        Need {
            fact,
            helper: self.offers(helper).then_some(helper),
        }
    }

    fn entry(&self, helper: Helper) -> Option<&Entry> {
        self.helpers.entry(&capability_of(helper)?)
    }

    /// Install `helper`. Resolves when PackageKit is done, or the person said no at the password
    /// prompt; a tool that arrives makes the registry read the plugins again before this returns,
    /// so the file that waited opens with it.
    pub async fn provide(&self, helper: Helper) -> HelperEnd {
        let Some(capability) = capability_of(helper) else {
            return HelperEnd::Unsupported(helper.slug().to_owned());
        };
        let end = match self.helpers.provide(&capability).await {
            Outcome::Installed => HelperEnd::Installed,
            Outcome::Declined => HelperEnd::Declined,
            Outcome::NotFound(missing) => HelperEnd::NotFound(looked_for(&missing)),
            Outcome::Unsupported(missing) => HelperEnd::Unsupported(needed_on_path(&missing)),
            Outcome::Failed(reason) => HelperEnd::Failed(reason),
        };
        if end == HelperEnd::Installed {
            self.read_plugins().await;
        }
        end
    }

    /// Look for every tool again, and tell the subscribers of the ones that changed: a tool the
    /// person installed with a package manager of their own appears here.
    pub fn look_again(&self) {
        for capability in self.capabilities() {
            self.helpers.refresh(&capability);
        }
    }

    fn capabilities(&self) -> Vec<Capability> {
        Helper::ALL
            .iter()
            .filter_map(|h| capability_of(*h))
            .collect()
    }

    async fn read_plugins(&self) {
        let registry = self.registry.clone();
        // A lost task leaves the registry as it was, which is the safe answer.
        let _read = tokio::task::spawn_blocking(move || registry.refresh()).await;
    }

    /// A window's end of the host: `told` is called, from any thread, with each tool that
    /// appears. Dropping the result stops it.
    pub fn listen(self: &Arc<Self>, told: impl Fn(Helper) + Send + Sync + 'static) -> Listening {
        let mut listeners = self
            .listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let key = listeners.next;
        listeners.next += 1;
        listeners.told.insert(key, Arc::new(told));
        Listening {
            host: Arc::downgrade(self),
            key,
        }
    }

    /// Hear of each tool that appears from now on, and tell the windows. The plugins are read again
    /// first, so what a window does with the news finds the tool usable. Never returns while the
    /// host lives: the program spawns it on its runtime.
    pub fn following(self: &Arc<Self>) -> impl Future<Output = ()> + Send + use<> {
        // Looking once first records what is there now, so only changes after it are news.
        self.look_again();
        let mut feed = self.helpers.subscribe();
        let host = Arc::clone(self);
        async move {
            loop {
                let change = feed.next().await;
                if change.presence != Presence::Present {
                    continue;
                }
                let Some(helper) = helper_of(&change.capability) else {
                    continue;
                };
                host.read_plugins().await;
                host.tell(helper);
            }
        }
    }

    fn tell(&self, helper: Helper) {
        let told: Vec<Told> = self
            .listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .told
            .values()
            .cloned()
            .collect();
        for window in told {
            window(helper);
        }
    }
}

/// A window's listening: dropping it stops it.
pub struct Listening {
    host: Weak<HelperHost>,
    key: u64,
}

impl std::fmt::Debug for Listening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listening").finish_non_exhaustive()
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        if let Some(host) = self.host.upgrade() {
            host.listeners
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .told
                .remove(&self.key);
        }
    }
}

impl HelperSource for HelperHost {
    fn words(&self, helper: Helper) -> Option<HelperWords> {
        let entry = self.entry(helper)?;
        Some(HelperWords {
            app: APP.to_owned(),
            tool: entry.tool.clone(),
            purpose: entry.purpose.clone(),
        })
    }
}
