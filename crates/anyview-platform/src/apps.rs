//! Open With: the applications that can open a file of a type, and starting one on a file.

use crate::error::PlatformError;
use anyview_core::{FilePath, Mime};
use ds_core::word::Word;

/// An application's desktop entry id, such as `org.gnome.eog.desktop`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DesktopId(String);

impl DesktopId {
    /// `id` when it names a desktop entry: it ends in `.desktop` and holds no `/`.
    pub fn new(id: &str) -> Option<DesktopId> {
        let named = id
            .strip_suffix(".desktop")
            .is_some_and(|stem| !stem.is_empty());
        (named && !id.contains('/')).then(|| DesktopId(id.to_owned()))
    }

    /// The id with its `.desktop` suffix.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why an application is offered for a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Association {
    /// The person's or the system's chosen default.
    Default,
    /// Added to the type's associations by the person or the system.
    Added,
    /// The application's own entry lists the type.
    Declared,
}

/// An application offered by Open With.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    /// Which entry it is, and what [`AppsForType::open_with`] takes.
    pub id: DesktopId,
    /// The name to show.
    pub name: String,
    /// Why it is offered.
    pub association: Association,
}

/// Which applications open a type, and opening a file with one.
pub trait AppsForType {
    /// The applications that open `mime`, the default first, then the added associations, then
    /// the entries that declare the type. An entry that is hidden, or cannot be read, is left
    /// out. Blocking: the caller runs it on a worker.
    fn apps_for(&self, mime: &Mime) -> Vec<AppEntry>;

    /// Start `app` on `file`, and do not wait for it.
    fn open_with(&self, app: &DesktopId, file: &FilePath) -> Result<(), PlatformError>;

    /// Whether this implementation has the service behind it. An implementation that answers
    /// "not available" to every request says `false`, so the views never offer what it cannot do.
    fn present(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_desktop_file_names_are_ids() {
        const CASES: &[(&str, &str, bool)] = &[
            ("entry", "org.gnome.eog.desktop", true),
            ("no suffix", "eog", false),
            ("only the suffix", ".desktop", false),
            ("a path", "applications/eog.desktop", false),
        ];
        for (name, id, valid) in CASES {
            assert_eq!(DesktopId::new(id).is_some(), *valid, "{name}");
        }
    }
}
