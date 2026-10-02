//! What every window of the program shares, and what makes one window its own.

use super::opening::Opening;
use crate::host::Hosting;
use anyview_ui::Workers;
use ds::prelude::Appearance;
use std::sync::Arc;

/// The wiring all windows share: the workers their jobs run on and the host their requests go
/// to.
#[derive(Clone)]
pub struct Factory {
    /// The pool the views' work runs on.
    pub workers: Arc<dyn Workers>,
    /// What carries out a window's requests.
    pub hosting: Arc<dyn Hosting>,
    /// How the windows look.
    pub appearance: Appearance,
}

impl std::fmt::Debug for Factory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Factory").finish_non_exhaustive()
    }
}

/// One window: the shared wiring and the file it opens. A window root takes it as its props.
#[derive(Debug, Clone)]
pub struct Seed {
    /// The shared wiring.
    pub factory: Factory,
    /// The file and its neighbours.
    pub opening: Opening,
}

impl PartialEq for Seed {
    fn eq(&self, other: &Seed) -> bool {
        Arc::ptr_eq(&self.factory.hosting, &other.factory.hosting)
            && Arc::ptr_eq(&self.factory.workers, &other.factory.workers)
            && self.opening == other.opening
    }
}
