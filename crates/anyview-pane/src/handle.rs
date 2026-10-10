//! The pane's commands, handed to the host as data for its own palette.
//!
//! The host makes a [`PaneHandle`] with [`use_pane_handle`] and passes it to the pane as its
//! `handle`, the way a quire component takes the handle its owner made (`use_edit_handle`). The
//! owner makes it because the host's palette is a sibling of the pane, not its child: a context
//! flows down from the host, and a callback from the pane would hand the host a handle only after
//! its first render, with nothing for the host to read until then. A handle the host already holds
//! can be read in any render, is `Copy`, and subscribes the component that reads it.

use anyview_ui::{PaneLink, use_pane_link};
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

    /// The link the pane fills in.
    pub(crate) fn link(&self) -> PaneLink {
        self.link
    }
}
