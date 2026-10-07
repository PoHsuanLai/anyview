//! `mimeapps.list`: which applications the person and the system associate with a type. The
//! parser and the ordering are pure; the caller reads the files.

use crate::apps::{Association, DesktopId};
use anyview_core::Mime;
use std::collections::{HashMap, HashSet};

/// One `mimeapps.list` file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct MimeAppsFile {
    default: HashMap<String, Vec<DesktopId>>,
    added: HashMap<String, Vec<DesktopId>>,
    removed: HashMap<String, Vec<DesktopId>>,
}

/// Which section a line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Default,
    Added,
    Removed,
    /// Anything else, which is skipped.
    Other,
}

impl MimeAppsFile {
    /// The file's three sections. A line that is not `type=id;id;`, an id that is not a desktop
    /// file name and a section the spec does not define are skipped.
    pub(super) fn parse(text: &str) -> MimeAppsFile {
        let mut file = MimeAppsFile::default();
        let mut section = Section::Other;
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                section = match name {
                    "Default Applications" => Section::Default,
                    "Added Associations" => Section::Added,
                    "Removed Associations" => Section::Removed,
                    _ => Section::Other,
                };
                continue;
            }
            let Some((mime, ids)) = line.split_once('=') else {
                continue;
            };
            let ids: Vec<DesktopId> = ids
                .split(';')
                .filter_map(|id| DesktopId::new(id.trim()))
                .collect();
            let target = match section {
                Section::Default => &mut file.default,
                Section::Added => &mut file.added,
                Section::Removed => &mut file.removed,
                Section::Other => continue,
            };
            target
                .entry(mime.trim().to_ascii_lowercase())
                .or_default()
                .extend(ids);
        }
        file
    }
}

/// The applications `files` (highest precedence first) associate with `mime`: every default,
/// then every added association, each once, minus what a file of higher precedence removed.
pub(super) fn associated(files: &[MimeAppsFile], mime: &Mime) -> Vec<(DesktopId, Association)> {
    let mut removed: HashSet<DesktopId> = HashSet::new();
    let mut seen: HashSet<DesktopId> = HashSet::new();
    let mut defaults = Vec::new();
    let mut added = Vec::new();
    for file in files {
        let mut take = |from: &HashMap<String, Vec<DesktopId>>, why: Association| {
            let ids = from.get(mime.as_str()).into_iter().flatten();
            let fresh: Vec<_> = ids
                .filter(|id| !removed.contains(*id) && seen.insert((*id).clone()))
                .map(|id| (id.clone(), why))
                .collect();
            fresh
        };
        defaults.extend(take(&file.default, Association::Default));
        added.extend(take(&file.added, Association::Added));
        removed.extend(
            file.removed
                .get(mime.as_str())
                .into_iter()
                .flatten()
                .cloned(),
        );
    }
    defaults.extend(added);
    defaults
}

/// What any of `files` removes from `mime`'s associations, so the entries that declare the type
/// themselves are held back too.
pub(super) fn removed(files: &[MimeAppsFile], mime: &Mime) -> HashSet<DesktopId> {
    files
        .iter()
        .filter_map(|file| file.removed.get(mime.as_str()))
        .flatten()
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> DesktopId {
        DesktopId::new(text).unwrap()
    }

    fn mime() -> Mime {
        Mime::parse("image/png").unwrap()
    }

    #[test]
    fn sections_and_ids_are_read() {
        let file = MimeAppsFile::parse(
            "# a comment\n[Default Applications]\nimage/png=a.desktop;b.desktop;\n\
             Image/PNG=c.desktop\nbroken line\n[Added Associations]\nimage/png=d.desktop;\n\
             [Removed Associations]\nimage/png=e.desktop;not-an-entry;\n[Strange]\nimage/png=f.desktop\n",
        );
        assert_eq!(
            file.default["image/png"],
            vec![id("a.desktop"), id("b.desktop"), id("c.desktop")]
        );
        assert_eq!(file.added["image/png"], vec![id("d.desktop")]);
        assert_eq!(file.removed["image/png"], vec![id("e.desktop")]);
    }

    #[test]
    fn defaults_come_before_added_and_a_higher_file_removes_from_lower_ones() {
        let user = MimeAppsFile::parse(
            "[Default Applications]\nimage/png=a.desktop\n[Removed Associations]\nimage/png=c.desktop\n",
        );
        let system = MimeAppsFile::parse(
            "[Added Associations]\nimage/png=b.desktop;c.desktop;a.desktop\n\
             [Default Applications]\nimage/png=d.desktop\n",
        );
        assert_eq!(
            associated(&[user, system], &mime()),
            vec![
                (id("a.desktop"), Association::Default),
                (id("d.desktop"), Association::Default),
                (id("b.desktop"), Association::Added),
            ]
        );
    }

    #[test]
    fn a_type_with_no_line_has_no_associations() {
        let file = MimeAppsFile::parse("[Default Applications]\ntext/plain=a.desktop\n");
        assert!(associated(&[file], &mime()).is_empty());
    }
}
