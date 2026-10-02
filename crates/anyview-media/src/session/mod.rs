//! The player as a typestate. A [`Session`] owns one libmpv player and is `Idle` (nothing
//! asked for), `Opening` (a file was given and mpv has not said it is open) or `Loaded` (it
//! has). What only a loaded recording has (its tracks and chapters, a seek, the volume) exists
//! on `Session<Loaded>` and nowhere else, so asking an idle player for its tracks is a compile
//! error, not an empty answer. Moving between the states consumes the session.
//!
//! ```compile_fail,E0599
//! // An idle player has no tracks: the call is not there to make.
//! fn tracks_of_nothing(session: anyview_media::Session<anyview_media::Idle>) {
//!     let _ = session.tracks();
//! }
//! ```
//!
//! ```compile_fail,E0599
//! // Nor can a file that is still opening be seeked.
//! fn seek_too_soon(session: anyview_media::Session<anyview_media::Opening>) {
//!     let _ = session.seek(anyview_core::MediaTime::default());
//! }
//! ```
//!
//! A session is `Send` and not `Sync`, and `poll` must be called by whoever owns it: the
//! media thread, which is not the thread that presents (FINDINGS, "`Player::poll` works from a
//! thread that does not present").

mod convert;
mod idle;
mod loaded;
mod opening;
mod options;
mod report;

pub use idle::Refused;
pub use opening::Opened;
pub use options::AudioDriver;
pub use report::{Frame, Report};

use crate::command::PictureSlot;
use crate::error::MediaError;
use anyview_core::MediaLength;
use mpv_wgpu_player::Player;

/// A session with nothing asked for yet.
#[derive(Debug, Clone, Copy)]
pub struct Idle;

/// A session whose file was handed to mpv and has not finished opening.
#[derive(Debug, Clone, Copy)]
pub struct Opening;

/// A session whose file is open.
#[derive(Debug, Clone, Copy)]
pub struct Loaded {
    pub(super) length: Option<MediaLength>,
}

/// One libmpv player, in the state `S`.
pub struct Session<S> {
    player: Player,
    state: S,
}

impl<S> std::fmt::Debug for Session<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl<S> Session<S> {
    /// Call `wake` whenever mpv wants attention. It may run on an mpv thread and must only
    /// wake the owner, who then calls `poll`.
    pub fn set_notify(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.player.set_notify(wake);
    }

    /// Where the picture goes. A slot of another size makes the player a new texture, which
    /// `picture` then shows.
    pub fn set_slot(&mut self, slot: PictureSlot) -> Result<(), MediaError> {
        self.player.set_slot(convert::slot(slot)).map_err(fault)
    }

    /// The texture the player draws into, once it has drawn.
    pub fn picture(&self) -> Option<&wgpu::TextureView> {
        match self.player.picture() {
            mpv_wgpu_player::Picture::Shown(view) => Some(view),
            mpv_wgpu_player::Picture::Waiting => None,
        }
    }

    fn with_state<T>(self, state: T) -> Session<T> {
        Session {
            player: self.player,
            state,
        }
    }
}

/// A failure of the player as this crate reports it.
pub(super) fn fault(error: mpv_wgpu_player::Error) -> MediaError {
    MediaError::Player(error.to_string())
}
