//! The program's media: the player on its own thread, the hub that keeps the desktop's
//! now-playing entry and carries its controls out, the sessions that play with no window, and the
//! exports of recordings. `anyview-media` is the player and libav; the window's views know only
//! `anyview_ui::MediaHost`, which `PlayerHost` implements.

mod actor;
mod exports;
mod host;
mod hub;
mod line;
mod map;
mod now_playing;
mod orders;
mod sink;
mod snapshot;

pub use exports::{ExportEnd, ExportHandle, Exports};
pub use host::PlayerHost;
pub use hub::MediaHub;
pub use now_playing::NowPlaying;
