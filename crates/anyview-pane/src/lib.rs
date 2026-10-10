//! The viewer as a pane another quire app hosts in its own window.
//!
//! [`ViewerPane`] is a component a host draws in a region of its window, over a [`PaneEdge`] that
//! carries the workers and the seams it has an implementation for. It is the viewer's window
//! without the window: no titlebar or traffic lights, no welcome window, no sheets, no palette
//! and no file drop, and none of the viewer's chords, since the host owns the keyboard. What the
//! pane asks of its host (to close it, to open a file elsewhere, to carry out a change to the
//! file) arrives as a [`PaneRequest`]; what it can do arrives as commands the host lists in its
//! own palette ([`PaneHandle::commands`]) and runs ([`PaneHandle::run`]).
//!
//! The pane never asks the OS for anything: no D-Bus, no player, no file chooser. The platform
//! abilities of a [`PaneEdge::portable`] edge are none, so Open, Print, Share, Open With and Show
//! in Folder are simply not there. A recording opens in a viewer window of its own, which the
//! host hears as [`PaneRequest::OpenElsewhere`].
//!
//! ```ignore
//! let focused = use_signal(|| true);
//! let handle = use_pane_handle();
//! let edge = use_hook(|| PaneEdge::portable(workers.clone()).with_resume_source(store.clone()));
//! rsx! {
//!     ViewerPane {
//!         file,
//!         edge,
//!         focused: focused,
//!         handle,
//!         on_request: move |request| terminal.handle(request),
//!     }
//! }
//! // In the host's palette: `handle.commands()` is a `PaletteGroup<PaneCommand>`, and a row
//! // picked is `handle.run(command)`.
//! ```
//!
//! Every public item is reached from this root, once.

#![warn(missing_docs)]

mod edge;
mod handle;
mod pane;
mod request;

pub use edge::PaneEdge;
pub use handle::{PaneCommand, PaneHandle, use_pane_handle};
pub use pane::{PaneProps, ViewerPane};
pub use request::{FileRequest, PaneRequest};

// The seams and the vocabulary a host names to wire a pane, so it depends on this crate alone.
pub use anyview_core::{FilePath, NonEmpty, Resume, Sequence, SequenceOrigin};
pub use anyview_peek::StillSource;
pub use anyview_ui::{
    DesktopService, EditRequest, ExportDraft, FileAccess, FileLocks, HelperSource, ImagePlugins,
    Look, LookFeed, MediaHost, NaturalSize, Opened, PaneChrome, PlatformAbilities, ResumeSource,
    Rewind, TextSave, TypedText, VersionKey, VersionSource, Work, WorkKind, WorkLane, Workers,
};
