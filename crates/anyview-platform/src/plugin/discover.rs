//! Finding the plugins: the manifests under `anyview/plugins` in the data directories, and
//! whether the programs they name are there.

use crate::env::Env;
use anyview_plugin::{Candidate, Manifest, Origin, PathRole, PluginError, Plugins, Readiness};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// What was found: the registry, and each file that could not become a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    /// The plugins that can be used, and the ones that were found and cannot be.
    pub plugins: Plugins,
    /// Manifest files that were unreadable, malformed or misnamed.
    pub rejected: Vec<Rejected>,
}

/// A manifest file that is not a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// The file.
    pub file: PathBuf,
    /// Why not.
    pub error: PluginError,
}

/// The folder under a data directory that holds the manifests.
const FOLDER: &str = "anyview/plugins";

/// Reads every manifest in `$XDG_DATA_HOME/anyview/plugins` (the person's) and in each of
/// `$XDG_DATA_DIRS` (the system's, most important first), as `env` names them. A file that is
/// not a usable manifest is listed in `rejected` and skipped; nothing here fails.
pub fn discover(env: &Env) -> Discovery {
    let dirs = std::iter::once((Origin::User, &env.dirs.data))
        .chain(env.dirs.data_dirs.iter().map(|dir| (Origin::System, dir)));
    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    for (origin, dir) in dirs {
        for file in manifest_files(&dir.join(FOLDER)) {
            match read_candidate(&file, origin) {
                Ok(candidate) => candidates.push(candidate),
                Err(error) => rejected.push(Rejected { file, error }),
            }
        }
    }
    Discovery {
        plugins: Plugins::resolve(candidates),
        rejected,
    }
}

/// The `.toml` files directly inside `folder`, by name; none when it cannot be listed.
fn manifest_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml") && path.is_file())
        .collect();
    files.sort();
    files
}

fn read_candidate(file: &Path, origin: Origin) -> Result<Candidate, PluginError> {
    let text =
        fs::read_to_string(file).map_err(|error| PluginError::Unreadable { kind: error.kind() })?;
    let manifest = Manifest::parse(&text)?;
    manifest.check_file_name(file)?;
    let readiness = readiness(&manifest);
    Ok(Candidate {
        manifest,
        origin,
        readiness,
    })
}

/// The first path the manifest names that is missing or cannot be run, or `Ready`.
fn readiness(manifest: &Manifest) -> Readiness {
    let problem = manifest
        .paths()
        .into_iter()
        .find_map(|(path, role)| check(path, role).err());
    match problem {
        Some(error) => Readiness::Unready(error),
        None => Readiness::Ready,
    }
}

fn check(path: &Path, role: PathRole) -> Result<(), PluginError> {
    let meta = fs::metadata(path).map_err(|_| PluginError::FileMissing {
        path: path.to_path_buf(),
    })?;
    let runnable = meta.is_file() && meta.permissions().mode() & 0o111 != 0;
    match role {
        PathRole::Executable if !runnable => Err(PluginError::NotExecutable {
            path: path.to_path_buf(),
        }),
        PathRole::Library if !meta.is_file() => Err(PluginError::FileMissing {
            path: path.to_path_buf(),
        }),
        PathRole::Executable | PathRole::Library => Ok(()),
    }
}
