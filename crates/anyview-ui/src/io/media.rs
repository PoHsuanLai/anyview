//! What the binary lends the window to play recordings with. The player (libmpv, its thread and
//! its texture) is the binary's: the window asks `MediaHost` to start one for a file, holds the
//! `MediaLine` it gets back, sends it the stage machine's commands and reads what it reports.
//! Nothing here names a player, so the views and the machines stay free of it.

use super::workers::Reply;
use crate::stage::{MediaError, PlayerCommand, PlayerEvent};
use crate::{Done, OpenError, Ticket};
use anyview_core::{
    Facts, FilePath, MediaChapter, MediaLength, MediaTags, MediaTime, MediaTrack, Sniffed, Source,
    Speed, VideoPresence,
};
use ds_blitz::TextureHandle;
use std::fmt::Debug;
use std::num::NonZeroU32;
use std::sync::Arc;

/// The size of the picture the player is to draw, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotPixels {
    /// Width.
    pub width: NonZeroU32,
    /// Height.
    pub height: NonZeroU32,
}

/// What the player says, in the stage machine's terms. The binary translates the player's own
/// events into these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaNotice {
    /// Something the stage machine hears.
    Player(PlayerEvent),
    /// Where playback is.
    Position(MediaTime),
    /// The track list, with the tracks that are playing.
    Tracks(Vec<MediaTrack>),
    /// The chapter list.
    Chapters(Vec<MediaChapter>),
    /// The speed.
    Speed(Speed),
    /// Whether a picture shows.
    Picture(VideoPresence),
    /// The player could not play the recording.
    Failed(MediaError),
}

/// Tells the window that its line has news. It posts one result to the window's mailbox and
/// returns at once, from any thread; the window then drains the line.
#[derive(Clone)]
pub struct MediaWake(Arc<dyn Fn() + Send + Sync>);

impl MediaWake {
    /// A wake that runs `wake`: the window's posts to its mailbox, a test's counts.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> MediaWake {
        MediaWake(Arc::new(wake))
    }

    /// Say that the line has news.
    pub fn wake(&self) {
        (self.0)();
    }
}

impl Debug for MediaWake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaWake").finish_non_exhaustive()
    }
}

/// What starting a player needs: the file, the texture the picture goes into, and how to wake
/// the window.
#[derive(Debug, Clone)]
pub struct MediaStart {
    /// The recording.
    pub file: FilePath,
    /// The file as the load read it: what the host reads the recording's facts from when no
    /// plugin does.
    pub source: Source,
    /// What the load found the file to be.
    pub sniffed: Sniffed,
    /// Where the picture is shown. The player's thread updates it and asks for a redraw.
    pub texture: TextureHandle,
    /// How the player tells the window it has news.
    pub wake: MediaWake,
}

/// A started player, as the window holds it. Dropping the last holder ends the player.
pub trait MediaLine: Debug + Send + Sync + 'static {
    /// Tell the player to do something. Safe from the UI thread: it never blocks.
    fn send(&self, command: PlayerCommand);
    /// Where the picture goes: the size of the stage, or `None` for no picture.
    fn resize(&self, slot: Option<SlotPixels>);
    /// What the player reported since the last call, oldest first.
    fn drain(&self) -> Vec<MediaNotice>;
}

/// A player that was started.
#[derive(Debug, Clone)]
pub struct MediaStarted {
    /// The line to it.
    pub line: Arc<dyn MediaLine>,
    /// What the file says of itself.
    pub tags: MediaTags,
    /// The facts the host read: how long it runs, its streams and codecs.
    pub facts: Facts,
    /// How long it runs, when the host knows before the player does.
    pub length: Option<MediaLength>,
}

/// Starts players. The binary implements it over its media thread.
pub trait MediaHost: Debug + Send + Sync + 'static {
    /// Start playing `start.file`. Blocking, briefly: it makes a thread and a player.
    fn start(&self, start: MediaStart) -> Result<MediaStarted, OpenError>;
}

/// The host a window has until the binary lends it a real one: it cannot play.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoPlayer;

impl MediaHost for NoPlayer {
    fn start(&self, _start: MediaStart) -> Result<MediaStarted, OpenError> {
        Err(OpenError::Media("this window has no player".to_owned()))
    }
}

/// The host and the way back from it, for one open.
#[derive(Debug, Clone)]
pub(crate) struct MediaPort {
    pub(crate) host: Arc<dyn MediaHost>,
    pub(crate) reply: Reply,
}

impl MediaPort {
    /// Start the player for `ticket`'s load. Its wake posts `Done::Media` for that ticket.
    pub(crate) fn start(
        &self,
        ticket: Ticket,
        source: &Source,
        sniffed: &Sniffed,
        texture: TextureHandle,
    ) -> Result<MediaStarted, OpenError> {
        let reply = self.reply.clone();
        let wake = MediaWake::new(move || reply.post(Done::Media { ticket }));
        self.host.start(MediaStart {
            file: source.path().clone(),
            source: source.clone(),
            sniffed: sniffed.clone(),
            texture,
            wake,
        })
    }
}
