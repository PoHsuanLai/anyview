//! The plugins as the program knows them now. The registry is read from disk when the program
//! starts and again when a tool a plugin runs is installed (the mpv plugin's manifest names the
//! person's mpv, which a package install makes usable), so a file that needed the tool opens with
//! it without the program restarting.

use anyview_plugin::Plugins;
use std::sync::{Arc, PoisonError, RwLock};

/// What reads the plugins again: discovery, or, in a test, a scripted answer.
type Reread = Arc<dyn Fn() -> Plugins + Send + Sync>;

/// The current registry, shared by every part of the program that routes a request through a
/// plugin. Cheap to clone; clones see the same registry.
#[derive(Clone)]
pub struct PluginRegistry {
    current: Arc<RwLock<Arc<Plugins>>>,
    reread: Reread,
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry").finish_non_exhaustive()
    }
}

impl Default for PluginRegistry {
    fn default() -> PluginRegistry {
        PluginRegistry::fixed(Plugins::none())
    }
}

impl PluginRegistry {
    /// A registry that starts as `plugins` and becomes what `reread` finds each time it is
    /// refreshed.
    pub fn rereading(
        plugins: Plugins,
        reread: impl Fn() -> Plugins + Send + Sync + 'static,
    ) -> PluginRegistry {
        PluginRegistry {
            current: Arc::new(RwLock::new(Arc::new(plugins))),
            reread: Arc::new(reread),
        }
    }

    /// A registry that is always `plugins`: refreshing finds the same ones.
    pub fn fixed(plugins: Plugins) -> PluginRegistry {
        let again = plugins.clone();
        PluginRegistry::rereading(plugins, move || again.clone())
    }

    /// The plugins now. A caller holds one answer for the whole of a request, so a refresh in the
    /// middle of it changes nothing it is already doing.
    pub fn current(&self) -> Arc<Plugins> {
        Arc::clone(&self.current.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// Read the plugins again. Blocking: it reads the manifests from disk.
    pub fn refresh(&self) {
        let found = Arc::new((self.reread)());
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = found;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_refresh_replaces_what_the_registry_holds_and_a_held_answer_is_unchanged() {
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let registry = PluginRegistry::rereading(Plugins::none(), move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Plugins::none()
        });
        let before = registry.current();
        registry.refresh();
        let after = registry.current();
        assert_eq!(reads.load(Ordering::SeqCst), 1, "one read for one refresh");
        assert!(
            !Arc::ptr_eq(&before, &after),
            "the registry holds the new answer"
        );
        assert_eq!(before.installed().len(), 0, "the held answer is as it was");
    }
}
