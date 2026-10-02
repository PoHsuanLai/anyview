//! Open With on Linux: `mimeapps.list` for the person's choices, then each desktop entry's own
//! `MimeType=` for the rest.

mod mimeapps;

use crate::apps::{AppEntry, AppsForType, Association, DesktopId};
use crate::env::Env;
use crate::error::PlatformError;
use crate::spawn::Argv;
use anyview_core::{FilePath, Mime};
use freedesktop_desktop_entry::DesktopEntry;
use mimeapps::{MimeAppsFile, associated, removed};
use std::collections::HashSet;
use std::path::PathBuf;

/// No locales: names are the entries' untranslated ones.
const NO_LOCALES: &[&str] = &[];

/// [`AppsForType`] over the freedesktop application entries.
#[derive(Debug, Clone)]
pub struct DesktopApps {
    env: Env,
}

impl DesktopApps {
    /// Open With reading `env`'s directories.
    pub fn new(env: Env) -> Self {
        DesktopApps { env }
    }

    /// The `mimeapps.list` files, highest precedence first: the user's and the system's
    /// configuration, then the same names under `applications/` in the data directories.
    fn mimeapps_files(&self) -> Vec<MimeAppsFile> {
        let dirs = &self.env.dirs;
        let config = std::iter::once(&dirs.config).chain(&dirs.config_dirs);
        let data = std::iter::once(&dirs.data)
            .chain(&dirs.data_dirs)
            .map(|dir| dir.join("applications"));
        config
            .cloned()
            .chain(data)
            .filter_map(|dir| std::fs::read_to_string(dir.join("mimeapps.list")).ok())
            .map(|text| MimeAppsFile::parse(&text))
            .collect()
    }

    /// `<data>/applications` for the person's data directory, then the system's.
    fn application_dirs(&self) -> Vec<PathBuf> {
        let dirs = &self.env.dirs;
        std::iter::once(&dirs.data)
            .chain(&dirs.data_dirs)
            .map(|dir| dir.join("applications"))
            .collect()
    }

    /// The entry named `id`, from the first applications directory that has it.
    fn entry(&self, id: &DesktopId) -> Option<DesktopEntry> {
        self.application_dirs()
            .into_iter()
            .map(|dir| dir.join(id.as_str()))
            .find_map(|path| DesktopEntry::from_path(path, Some(NO_LOCALES)).ok())
    }

    /// The listed entry for `id` as an offer, unless it is hidden or has nothing to run.
    fn offer(&self, id: DesktopId, association: Association) -> Option<AppEntry> {
        let entry = self.entry(&id)?;
        let name = entry.name(NO_LOCALES)?.into_owned();
        offered(&entry).then_some(AppEntry {
            id,
            name,
            association,
        })
    }

    /// Every entry whose `MimeType=` lists `mime`, in directory order.
    fn declared(&self, mime: &Mime) -> Vec<DesktopId> {
        let dirs = self.application_dirs();
        freedesktop_desktop_entry::Iter::new(dirs.into_iter())
            .filter_map(|path| DesktopEntry::from_path(path, Some(NO_LOCALES)).ok())
            .filter(|entry| {
                entry.mime_type().is_some_and(|types| {
                    types.iter().any(|t| t.eq_ignore_ascii_case(mime.as_str()))
                })
            })
            .filter_map(|entry| DesktopId::new(&format!("{}.desktop", entry.id())))
            .collect()
    }
}

/// Whether an entry is something the person may be offered.
fn offered(entry: &DesktopEntry) -> bool {
    !entry.hidden() && !entry.no_display() && entry.exec().is_some()
}

impl AppsForType for DesktopApps {
    fn apps_for(&self, mime: &Mime) -> Vec<AppEntry> {
        let files = self.mimeapps_files();
        let removed = removed(&files, mime);
        let declared = self
            .declared(mime)
            .into_iter()
            .filter(|id| !removed.contains(id))
            .map(|id| (id, Association::Declared));
        let mut seen = HashSet::new();
        associated(&files, mime)
            .into_iter()
            .chain(declared)
            .filter(|(id, _)| seen.insert(id.clone()))
            .filter_map(|(id, association)| self.offer(id, association))
            .collect()
    }

    fn open_with(&self, app: &DesktopId, file: &FilePath) -> Result<(), PlatformError> {
        let exec = |reason: String| PlatformError::Exec {
            id: app.as_str().to_owned(),
            reason,
        };
        let entry = self
            .entry(app)
            .ok_or_else(|| exec("no such desktop entry".to_owned()))?;
        let path = file.as_path().to_string_lossy();
        let words = entry
            .parse_exec_with_uris(&[path.as_ref()], NO_LOCALES)
            .map_err(|error| exec(error.to_string()))?;
        let argv =
            Argv::from_words(words).ok_or_else(|| exec("the command is empty".to_owned()))?;
        self.env.spawn.spawn(&argv)
    }
}

#[cfg(test)]
mod tests;
