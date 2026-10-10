//! The pane's commands, handed to the host as data for its own palette.
//!
//! The host makes a [`PaneHandle`] with [`use_pane_handle`] and passes it to the pane as its
//! `handle`, the way a quire component takes the handle its owner made (`use_edit_handle`). The
//! owner makes it because the host's palette is a sibling of the pane, not its child: a context
//! flows down from the host, and a callback from the pane would hand the host a handle only after
//! its first render, with nothing for the host to read until then. A handle the host already holds
//! can be read in any render, is `Copy`, and subscribes the component that reads it.
//!
//! Find is the host's palette too. ⌘F is the host's chord: it opens its palette in a "find in
//! pane" mode and feeds each query typed to [`PaneHandle::find`]; the matches come back as rows
//! from [`PaneHandle::hits`], and picking one is [`PaneHandle::run`]. The pane marks the hits in
//! the file and goes to the current one; [`PaneHandle::end_find`] puts the find away.
//!
//! ```ignore
//! // In the host's palette component, over a query it owns as a signal:
//! let handle = use_pane_handle();
//! use_effect(move || handle.find(query()));          // an empty query ends the find
//! let hits = handle.hits(list());                    // HitList::Brief, or Whole after Show All
//! // ...show `hits` as a group; on a picked row:
//! match command {
//!     PaneCommand::ShowAllHits => list.set(HitList::Whole),
//!     hit => handle.run(hit),                        // the pane goes to that hit
//! }
//! // When the host closes its find mode:
//! handle.end_find();
//! ```

use anyview_ui::{HitList, PaneLink, TypedText, use_pane_link};
use ds::components::lists::row::chord::RowChord;
use ds::components::menus::palette::palette_group::{PaletteGroup, PaletteRow};

/// One thing the pane can do: a row of the viewer's own palette (Zoom to Fit, Rotate Left, Share,
/// Move to Trash…), with its label and its keys.
pub type PaneCommand = anyview_ui::Command;

/// What a host holds of one pane: the commands the open file offers now, and a way to run one.
/// Make it with [`use_pane_handle`] and hand it to the pane. Until a pane shows a file the group
/// is empty, and running a command does nothing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaneHandle {
    link: PaneLink,
}

/// A handle with nothing listed and no pane behind it yet. Call it in the host component that
/// owns the pane.
pub fn use_pane_handle() -> PaneHandle {
    PaneHandle {
        link: use_pane_link(),
    }
}

impl PaneHandle {
    /// The commands the open file offers, as one palette group titled with the file's name and
    /// each row carrying its keys. A host merges it beside its own groups (`GroupOrder` orders
    /// them) and runs a picked row with [`PaneHandle::run`]. Reading it in a component re-renders
    /// that component when the stage changes what is on offer.
    #[must_use]
    pub fn commands(&self) -> PaletteGroup<PaneCommand> {
        let rows = self
            .link
            .commands()
            .into_iter()
            .map(|command| PaletteRow {
                chord: command.shortcut().map(RowChord::always).unwrap_or_default(),
                ..PaletteRow::new(command, command.label())
            })
            .collect();
        PaletteGroup::list(self.link.title(), rows)
    }

    /// Run `command` as the viewer's own palette would run the row picked: through the machine's
    /// one command path, so a command the file no longer offers does nothing. What it changes in
    /// the file arrives as a [`PaneRequest::File`](crate::PaneRequest::File).
    pub fn run(&self, command: PaneCommand) {
        self.link.run(command);
    }

    /// Search the open file for `query`, as the viewer's own find does: the pane marks the hits
    /// and goes to the one nearest the place it shows, and [`PaneHandle::hits`] lists them once
    /// the search has run. Call it with each query the person types; a new query replaces the
    /// last, and an empty one ends the find. A file that cannot be searched (a picture) has no
    /// hits, and nothing happens before a pane shows a file.
    pub fn find(&self, query: impl Into<String>) {
        self.link.find(&TypedText::new(query));
    }

    /// Put the find away: its marks leave the file and [`PaneHandle::hits`] lists none. The host
    /// calls it when it closes its find mode (picking a hit leaves the find up, so the marks stay
    /// on the file until then).
    pub fn end_find(&self) {
        self.link.end_find();
    }

    /// The hits of the find that is up as one palette group titled "In This File": each row shows
    /// the words around the match with the match marked, and where it is (`Line 42`, `Page 7`),
    /// and yields a `FindHit` command that [`PaneHandle::run`] goes to. The group is empty
    /// while no find is up and when nothing matches. [`HitList::Brief`] lists the first few hits
    /// and, when there are more, a last row yielding `ShowAllHits`; running that
    /// does nothing, so the host answers it by listing [`HitList::Whole`] (up to 200). Reading it
    /// in a component re-renders that component as the search answers.
    #[must_use]
    pub fn hits(&self, list: HitList) -> PaletteGroup<PaneCommand> {
        PaletteGroup::list("In This File", self.link.hits(list))
    }

    /// The link the pane fills in.
    pub(crate) fn link(&self) -> PaneLink {
        self.link
    }
}
