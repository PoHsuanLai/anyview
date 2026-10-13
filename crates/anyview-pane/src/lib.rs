//! The viewer as a pane another quire app hosts in its own window.
//!
//! [`ViewerPane`] is a component a host draws in a region of its window, over a [`PaneEdge`] that
//! carries the workers and the seams it has an implementation for. It is the viewer's window
//! without the window: no titlebar or traffic lights, no welcome window, no sheets, no palette
//! and no file drop, and none of the viewer's chords, since the host owns the keyboard. What the
//! pane asks of its host (to close it, to open a file elsewhere, to carry out a change to the
//! file) arrives as a [`PaneRequest`]; what it can do arrives as commands the host lists in its
//! own palette ([`PaneHandle::commands`]) and runs ([`PaneHandle::run`]); a find in the file is the
//! host's palette too ([`PaneHandle::find`], [`PaneHandle::hits`]).
//!
//! A file the pane could not open (unreadable, damaged, locked, empty, too large or of a kind
//! nothing here shows) is [`PaneRequest::NotOpened`], with the file and the [`LoadFailure`] its own
//! card names; the card is the pane's, and any other way to open the file is the host's to offer.
//!
//! The pane never asks the OS for anything: no D-Bus, no file chooser. The platform abilities of a
//! [`PaneEdge::portable`] edge are none, so Open, Print, Share, Open With and Show in Folder are
//! simply not there. A recording opens in a viewer window of its own, which the host hears as
//! [`PaneRequest::OpenElsewhere`], unless the host gave the edge a player ([`Seams::with_media`],
//! or [`PaneEdge::with_player`] with the feature `player`): then it plays in the pane.
//!
//! ```ignore
//! let focused = use_signal(|| true);
//! let handle = use_pane_handle();
//! let seams = Seams::default().with_resume_source(store.clone());
//! let edge = use_hook(|| PaneEdge::new(workers.clone(), seams));
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
mod store;

pub use edge::PaneEdge;
pub use handle::{PaneCommand, PaneHandle, use_pane_handle};
pub use pane::{PaneProps, ViewerPane};
pub use request::{FileRequest, PaneRequest};

// The seams and the vocabulary a host names to wire a pane, so it depends on this crate alone.
pub use anyview_core::{FilePath, NonEmpty, Resume, Sequence, SequenceOrigin};
#[cfg(feature = "player")]
pub use anyview_media::{AudioDriver, MpvHost};
#[cfg(feature = "player")]
pub use anyview_media_host::{AudioFocus, FixedMpv, MediaHub, PlayerHost, PlayerPlugins};
pub use anyview_peek::StillSource;
/// The folder under the person's data directory where the viewer's windows keep their memory:
/// give [`PaneEdge::with_store`] the same, so a pane shares one history with them.
pub use anyview_store::{Clock, STORE_FOLDER};
pub use anyview_ui::{
    DesktopService, EditRequest, ExportDraft, FileAccess, FileLocks, HelperSource, HitList,
    ImagePlugins, LoadFailure, Look, LookFeed, LookSender, MediaHost, MediaSupport, NaturalSize,
    NotOpened, Opened, PaneChrome, PlatformAbilities, ResumeSource, Rewind, Seams, TextSave,
    TypedText, VersionKey, VersionSource, Work, WorkKind, WorkLane, Workers,
};
