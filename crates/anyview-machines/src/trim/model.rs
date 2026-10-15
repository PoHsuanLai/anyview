//! The trim's state, inputs and what it can be asked.

use crate::stage::TrimEdge;
use anyview_core::{Edit, MediaLength, MediaTime};

/// The least a trim keeps, so that both handles can still be taken hold of.
pub const MIN_KEPT: MediaTime = MediaTime::from_millis(100);

/// A stretch of the recording, from `start` to `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrimWindow {
    /// Where the stretch begins.
    pub start: MediaTime,
    /// Where it ends; after `start`.
    pub end: MediaTime,
}

impl TrimWindow {
    /// `at`, or the nearest end of the stretch when it is outside.
    #[must_use]
    pub fn clamp(self, at: MediaTime) -> MediaTime {
        at.max(self.start).min(self.end)
    }

    /// How long the stretch runs.
    #[must_use]
    pub fn length(self) -> MediaLength {
        MediaLength(MediaTime(self.end.0.saturating_sub(self.start.0)))
    }
}

/// The handles of the bar while it is up: the recording's whole length, and the stretch between
/// the handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Choosing {
    pub(super) whole: MediaLength,
    pub(super) window: TrimWindow,
}

/// The open recording's trim. It is about the recording as it was opened, so a new one
/// (`Discard`, when it lands) starts it afresh.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaTrim {
    pub(super) choosing: Option<Choosing>,
    pub(super) applied: Option<TrimWindow>,
    pub(super) undone: Option<TrimWindow>,
}

/// What changes the trim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimIn {
    /// Trim… was chosen on a recording this long: the bar comes up on the stretch already
    /// applied, else on the whole recording. A recording too short to keep a part of is left
    /// as it is.
    Begin(MediaLength),
    /// A handle was moved to this time. It stops short of the other handle by [`MIN_KEPT`] and
    /// stays inside the recording.
    Move {
        /// Which handle.
        edge: TrimEdge,
        /// Where it was moved to.
        to: MediaTime,
    },
    /// Trim (Return): the stretch between the handles is the recording now. The whole recording
    /// is no trim at all.
    Apply,
    /// Cancel (Esc): the bar goes, and the trim is as it was before it came up.
    Cancel,
    /// Take back the applied trim.
    Undo,
    /// Do again the trim that was taken back.
    Redo,
    /// Let it go: the recording is as it was opened, or was just saved.
    Discard,
}

impl MediaTrim {
    /// Whether the bar is up.
    #[must_use]
    pub fn is_choosing(&self) -> bool {
        self.choosing.is_some()
    }

    /// Whether a trim is applied and not saved.
    #[must_use]
    pub fn is_edited(&self) -> bool {
        self.applied.is_some()
    }

    /// Whether an applied trim can be taken back.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.choosing.is_none() && self.applied.is_some()
    }

    /// Whether a trim that was taken back can be done again.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.choosing.is_none() && self.undone.is_some()
    }

    /// The recording's length and the stretch between the handles, while the bar is up.
    #[must_use]
    pub fn handles(&self) -> Option<(MediaLength, TrimWindow)> {
        self.choosing
            .map(|choosing| (choosing.whole, choosing.window))
    }

    /// The stretch applied, if one is.
    #[must_use]
    pub fn applied(&self) -> Option<TrimWindow> {
        self.applied
    }

    /// The stretch that plays: the one between the handles while the bar is up, else the one
    /// applied. Playback stays inside it.
    #[must_use]
    pub fn play_range(&self) -> Option<TrimWindow> {
        self.choosing
            .map(|choosing| choosing.window)
            .or(self.applied)
    }

    /// The edit that saves the applied trim.
    #[must_use]
    pub fn edit(&self) -> Option<Edit> {
        self.applied.map(|window| Edit::Trim {
            start: window.start,
            end: window.end,
        })
    }
}
