//! The host side of the player, for a window or for a pane in another application's window.
//!
//! [`PlayerHost`] is the `anyview_ui::MediaHost` the views start players with: each recording gets
//! a thread of its own (through `anyview-runtime`) that runs `anyview_media`'s driver, the person's
//! own mpv as a child process drawing into the view's texture through a memfd ring (Linux), or the
//! built-in audio player (feature `audio`). [`MediaHub`] keeps the sessions: the desktop's one
//! now-playing entry and the controls that come back from it, the sessions that play with no
//! window, a saved frame, and (for panes sharing a hub) one source of sound at a time
//! ([`AudioFocus`]). What it asks of the plugins is [`PlayerPlugins`]; a host without a plugin
//! registry answers with [`FixedMpv`].
//!
//! No library of codecs is linked: libmpv and libav are never in this tree (CONVENTIONS section 15).
//!
//! ```ignore
//! let hub = MediaHub::standalone(&runtime, AudioDriver::Auto, Arc::new(FixedMpv::new(mpv)));
//! let host = PlayerHost::new(hub);
//! // A pane: `PaneEdge::with_player(host)`.
//! ```
//!
//! Every public item is reached from this root, once.

#![warn(missing_docs)]

mod actor;
mod engine;
mod focus;
mod guard;
mod host;
mod hub;
mod line;
mod map;
mod orders;
mod plugins;
mod sink;
mod snapshot;

pub use focus::AudioFocus;
pub use host::PlayerHost;
pub use hub::{MediaHub, NoEntry, ShotError};
pub use plugins::{ExportTool, FixedMpv, PlayRoute, PlayerPlugins, Playing, Reading, WriteRoute};
