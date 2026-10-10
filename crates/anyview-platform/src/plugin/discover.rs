//! Finding the plugins: the manifests under `anyview/plugins` in the data directories, and
//! whether the programs they name are there. The search is bayonet's; this names the folder and
//! the directories.

use crate::env::Env;
use anyview_plugin::{Plugins, Provision};
use anyview_plugin_protocol::PROTOCOL_VERSION;
use bayonet::Search;
use std::path::Path;

/// A manifest file that is not a plugin.
pub type Rejected = bayonet::Rejected<Provision>;

/// What was found: the registry, and each file that could not become a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    /// The plugins that can be used, and the ones that were found and cannot be.
    pub plugins: Plugins,
    /// Manifest files that were unreadable, malformed or misnamed.
    pub rejected: Vec<Rejected>,
}

/// Reads every manifest in `$XDG_DATA_HOME/anyview/plugins` (the person's) and in each of
/// `$XDG_DATA_DIRS` (the system's, most important first), as `env` names them. A file that is
/// not a usable manifest is listed in `rejected` and skipped; nothing here fails.
pub fn discover(env: &Env) -> Discovery {
    discover_in(env, &env.dirs.data)
}

/// [`discover`] with `folder` as the person's data directory, whose `anyview/plugins` is read
/// first: a host that keeps its plugins elsewhere (a development tree, a portable install) names
/// the folder, and `env`'s system directories still follow.
pub fn discover_in(env: &Env, folder: &Path) -> Discovery {
    let found = bayonet::discover::<Provision>(&Search::new(
        "anyview",
        folder,
        &env.dirs.data_dirs,
        PROTOCOL_VERSION,
    ));
    Discovery {
        plugins: Plugins::from(found.registry),
        rejected: found.rejected,
    }
}
