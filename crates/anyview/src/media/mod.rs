//! The program's media: the player on its own thread, the hub that keeps the desktop's
//! now-playing entry and carries its controls out, the sessions that play with no window, and the
//! exports of recordings. `anyview-media` is the player and the plan of the exports, and the plugins read and write; the window's views know only
//! `anyview_ui::MediaHost`, which `PlayerHost` implements.

mod actor;
mod engine;
mod exports;
mod guard;
mod host;
mod hub;
mod line;
mod map;
mod now_playing;
mod orders;
mod plugins;
mod sink;
mod snapshot;

pub use exports::{ExportEnd, ExportHandle, Exports, PluginExport};
pub use host::PlayerHost;
pub use hub::MediaHub;
pub use now_playing::NowPlaying;
pub use plugins::{ExportTool, MediaPlugins, PlayRoute, Playing, Reading, WriteRoute};
