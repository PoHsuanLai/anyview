//! The program's media: the plugins that play, read and write recordings, the desktop's now-playing
//! entry, and the exports of recordings. The player on its own thread, the hub of sessions and the
//! `MediaHost` the window starts players with are `anyview-media-host`'s (the pane links them too),
//! re-exported here under the names the binary has always used; `anyview-media` is the player and
//! the plan of the exports.

mod exports;
mod now_playing;
mod plugins;

pub use anyview_media_host::{
    ExportTool, MediaHub, PlayRoute, PlayerHost, Playing, Reading, ShotError, WriteRoute,
};
pub use exports::{ExportEnd, ExportHandle, Exports, PluginExport};
pub use now_playing::NowPlaying;
pub use plugins::MediaPlugins;
