//! The window's line to its player: the media thread's actor, and the mailbox its news comes
//! back through.

use super::hub::{Inner, SessionId};
use super::map;
use crate::runtime::{Actor, Mailbox};
use anyview_media::{MediaCommand, PictureSlot};
use anyview_ui::{MediaLine, MediaNotice, PlayerCommand, SlotPixels};
use std::sync::Weak;

/// A running player. Dropping it ends the media thread, and the hub forgets the session.
pub(super) struct LiveLine {
    // Dropped after `drop` has told the hub, which joins the thread.
    pub(super) actor: Actor<MediaCommand>,
    pub(super) mailbox: Mailbox<MediaNotice>,
    pub(super) id: SessionId,
    pub(super) hub: Weak<Inner>,
}

impl std::fmt::Debug for LiveLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveLine")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl LiveLine {
    /// Tell the player. A player whose thread has ended takes nothing, and nobody is waiting.
    pub(super) fn command(&self, command: MediaCommand) {
        let _ended = self.actor.send(command);
    }
}

impl MediaLine for LiveLine {
    fn send(&self, command: PlayerCommand) {
        self.command(map::command_of(command));
    }

    fn resize(&self, slot: Option<SlotPixels>) {
        self.command(MediaCommand::Slot(match slot {
            Some(SlotPixels { width, height }) => PictureSlot::Sized { width, height },
            None => PictureSlot::Empty,
        }));
    }

    fn drain(&self) -> Vec<MediaNotice> {
        self.mailbox.drain()
    }
}

impl Drop for LiveLine {
    fn drop(&mut self) {
        if let Some(hub) = self.hub.upgrade() {
            hub.gone(self.id);
        }
    }
}
