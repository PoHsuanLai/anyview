//! The window's handle on the live state of the player it shows: a signal the window makes once
//! and writes when the player has news, and the stage, the capsule and the panel read.

use super::live::{MediaLive, MediaPlace};
use crate::MediaNotice;
use anyview_core::MediaTime;
use dioxus::prelude::*;

/// The live state of the recording the window shows. Copy: it is a signal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaShelf(Signal<MediaLive>);

/// The shelf of a window: made once, in the window's root.
pub fn use_media_shelf() -> MediaShelf {
    MediaShelf(use_signal(MediaLive::default))
}

impl MediaShelf {
    /// What the shelf holds, read where a change should redraw.
    pub fn read(&self) -> impl std::ops::Deref<Target = MediaLive> + '_ {
        self.0.read()
    }

    /// What the shelf holds, read without subscribing.
    pub fn peek(&self) -> impl std::ops::Deref<Target = MediaLive> + '_ {
        self.0.peek()
    }

    /// Forget the recording: a new file is being loaded.
    pub fn reset(&self) {
        let mut live = self.0;
        live.set(MediaLive::default());
    }

    /// Take in the player's news.
    pub fn apply(&self, notices: &[MediaNotice]) {
        let mut live = self.0;
        live.with_mut(|live| notices.iter().for_each(|notice| live.apply(notice)));
    }

    /// Where the person is and how they have it set, once the player has said.
    pub fn place(&self) -> Option<MediaPlace> {
        self.0.peek().place()
    }

    /// Mark where a trim begins or ends.
    pub fn mark(&self, edge: crate::TrimEdge, at: MediaTime) {
        let mut live = self.0;
        live.with_mut(|live| match edge {
            crate::TrimEdge::Start => live.marks.start = Some(at),
            crate::TrimEdge::End => live.marks.end = Some(at),
        });
    }
}
