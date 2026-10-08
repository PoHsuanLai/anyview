//! Finding the plugins: the manifests under `anyview/plugins` in the data directories, and
//! whether the programs they name are there. The search is bayonet's; this names the folder and
//! the directories.

use crate::env::Env;
use anyview_plugin::{Plugins, Provision};
use anyview_plugin_protocol::PROTOCOL_VERSION;
use bayonet::Search;

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
    let found = bayonet::discover::<Provision>(&Search {
        app: "anyview",
        user: &env.dirs.data,
        system: &env.dirs.data_dirs,
        protocol: PROTOCOL_VERSION,
    });
    Discovery {
        plugins: Plugins::from(found.registry),
        rejected: found.rejected,
    }
}
