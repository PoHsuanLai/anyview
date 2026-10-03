//! The desktop's now-playing entry, when the desktop has one to give.

use anyview_platform::linux::MprisSession;
use anyview_platform::{Env, MediaControl, MediaSession, MediaState, PlatformError};

/// The program's one now-playing entry: MPRIS on the session bus, or nothing when there is no
/// bus (the players still play; the desktop just cannot see them).
#[derive(Debug)]
pub enum NowPlaying {
    /// The viewer as an MPRIS player.
    Mpris(MprisSession),
    /// No entry: publishing is nothing, and no control ever arrives.
    Absent,
}

impl NowPlaying {
    /// Claim the player's name on `env`'s session bus; with no bus the entry is absent, and the
    /// reason is said once.
    pub async fn register(env: &Env) -> NowPlaying {
        match MprisSession::register(env).await {
            Ok(session) => NowPlaying::Mpris(session),
            Err(error) => {
                eprintln!("anyview: no now-playing entry for the desktop: {error}");
                NowPlaying::Absent
            }
        }
    }
}

impl MediaSession for NowPlaying {
    async fn publish(&self, state: &MediaState) -> Result<(), PlatformError> {
        match self {
            NowPlaying::Mpris(session) => session.publish(state).await,
            NowPlaying::Absent => Ok(()),
        }
    }

    async fn next_control(&mut self) -> Option<MediaControl> {
        match self {
            NowPlaying::Mpris(session) => session.next_control().await,
            NowPlaying::Absent => std::future::pending().await,
        }
    }
}
