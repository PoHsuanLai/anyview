//! The files opened ahead of the person: the neighbours of the open file, and the file just left,
//! each held as its open document so an arrow key shows it with no wait. Pure bookkeeping over
//! paths: the window decides when to open a neighbour and shows what is held; this says which
//! are wanted, which are held and which are on their way.

use crate::io::Preloaded;
use anyview_core::FilePath;

/// What is held and what is wanted.
#[derive(Debug, Clone, Default)]
pub(super) struct Preloads {
    wanted: Vec<FilePath>,
    held: Vec<Preloaded>,
    flying: Vec<FilePath>,
}

impl Preloads {
    /// The files `around` the open one are the ones wanted now: everything else held is let go,
    /// and the result is the files that must be opened because they are neither held nor already
    /// being opened (they are now counted as on their way).
    pub(super) fn want(&mut self, around: &[FilePath]) -> Vec<FilePath> {
        self.wanted = around.to_vec();
        self.held
            .retain(|held| around.contains(held.probed.source.path()));
        self.flying.retain(|path| around.contains(path));
        let fetch: Vec<FilePath> = around
            .iter()
            .filter(|path| !self.holds(path) && !self.flying.contains(path))
            .cloned()
            .collect();
        self.flying.extend(fetch.iter().cloned());
        fetch
    }

    /// A preload came back: `loaded` is held if its file is still wanted, and dropped otherwise
    /// (or when the open failed or was not worth doing).
    pub(super) fn arrived(&mut self, path: &FilePath, loaded: Option<Preloaded>) {
        self.flying.retain(|flying| flying != path);
        if let Some(loaded) = loaded
            && self.wanted.contains(path)
            && !self.holds(path)
        {
            self.held.push(loaded);
        }
    }

    /// The file just left, kept as it was left so that coming back to it is as quick. It stays
    /// until the next set of neighbours is wanted and does not name it.
    pub(super) fn stash(&mut self, left: Preloaded) {
        self.held
            .retain(|held| held.probed.source.path() != left.probed.source.path());
        self.held.push(left);
    }

    /// Let go of the held documents that lacked `helper` (a card that said a tool was missing):
    /// they open again, with the tool, when they are next wanted.
    pub(super) fn forget_lacking(&mut self, helper: anyview_core::Helper) {
        self.held
            .retain(|held| held.doc.view().lacks() != Some(helper));
    }

    /// The held document of `path`, handed over: it is the open file now.
    pub(super) fn take(&mut self, path: &FilePath) -> Option<Preloaded> {
        let at = self
            .held
            .iter()
            .position(|held| held.probed.source.path() == path)?;
        Some(self.held.remove(at))
    }

    fn holds(&self, path: &FilePath) -> bool {
        self.held
            .iter()
            .any(|held| held.probed.source.path() == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StageFamily;
    use crate::families::{LoadedDoc, PeekOnlyDoc, PeekOnlyStageView};
    use crate::io::{Opened, Readable};
    use anyview_core::{
        ByteLen, Facts, FileStamp, FormatKind, ModTime, Resume, Source, sniff_folder,
    };

    fn path(name: &str) -> FilePath {
        FilePath::new(format!("/p/{name}")).unwrap()
    }

    fn preloaded(name: &str) -> Preloaded {
        let stamp = FileStamp {
            len: ByteLen(1),
            modified: ModTime(0),
        };
        Preloaded {
            probed: Opened {
                source: Source::new(path(name), stamp),
                sniffed: sniff_folder(),
                family: StageFamily::PeekOnly,
                resume: Resume::Nothing,
                access: crate::FileAccess::Writable,
            },
            doc: LoadedDoc::of::<PeekOnlyStageView>(PeekOnlyDoc {
                name: name.to_owned(),
                kind: FormatKind::Folder,
                facts: Facts::empty(),
                thumbnail: None,
                listing: Vec::new(),
                readable: Readable::Yes,
            }),
        }
    }

    fn names(paths: &[FilePath]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().as_str().to_owned())
            .collect()
    }

    #[test]
    fn neighbours_are_fetched_once_and_a_file_already_on_its_way_is_not_asked_again() {
        let mut preloads = Preloads::default();
        assert_eq!(names(&preloads.want(&[path("a"), path("c")])), ["a", "c"]);
        assert_eq!(
            names(&preloads.want(&[path("a"), path("c")])),
            Vec::<String>::new(),
            "both are already on their way"
        );
        assert_eq!(
            names(&preloads.want(&[path("c"), path("e")])),
            ["e"],
            "only the new one is fetched"
        );
    }

    #[test]
    fn a_file_that_arrives_is_held_only_while_it_is_wanted() {
        let mut preloads = Preloads::default();
        preloads.want(&[path("a"), path("c")]);
        preloads.arrived(&path("a"), Some(preloaded("a")));
        assert!(preloads.take(&path("a")).is_some(), "wanted, so held");
        preloads.want(&[path("c"), path("e")]);
        preloads.arrived(&path("a"), Some(preloaded("a")));
        assert!(
            preloads.take(&path("a")).is_none(),
            "no longer wanted when it came"
        );
    }

    #[test]
    fn a_failed_preload_is_asked_for_again_next_time() {
        let mut preloads = Preloads::default();
        preloads.want(&[path("a")]);
        preloads.arrived(&path("a"), None);
        assert_eq!(names(&preloads.want(&[path("a")])), ["a"]);
    }

    #[test]
    fn the_file_just_left_is_held_until_the_neighbours_do_not_name_it() {
        let mut preloads = Preloads::default();
        preloads.stash(preloaded("b"));
        assert_eq!(
            names(&preloads.want(&[path("b"), path("d")])),
            ["d"],
            "b is a neighbour of where we went, so it is already held"
        );
        assert!(preloads.take(&path("b")).is_some());
        preloads.stash(preloaded("b"));
        preloads.want(&[path("x"), path("y")]);
        assert!(
            preloads.take(&path("b")).is_none(),
            "a jump far away lets it go"
        );
    }

    #[test]
    fn stashing_a_file_twice_holds_it_once() {
        let mut preloads = Preloads::default();
        preloads.stash(preloaded("b"));
        preloads.stash(preloaded("b"));
        assert!(preloads.take(&path("b")).is_some());
        assert!(preloads.take(&path("b")).is_none());
    }
}
