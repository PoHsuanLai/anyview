//! Why the chrome is held up. Reasons stack, so releasing one of two leaves it held.

use ds_core::word::Word;

/// One reason to keep the chrome on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PinReason {
    /// The pointer is over the capsule or the titlebar.
    PointerOverCapsule,
    /// A menu or the palette is open.
    MenuOpen,
    /// Media is paused, so its controls stay where the person left them.
    MediaPaused,
    /// Keyboard focus is inside the chrome.
    KeyboardFocus,
}

impl PinReason {
    const fn bit(self) -> u8 {
        match self {
            PinReason::PointerOverCapsule => 1,
            PinReason::MenuOpen => 2,
            PinReason::MediaPaused => 4,
            PinReason::KeyboardFocus => 8,
        }
    }
}

/// A set of [`PinReason`]s that is never empty: `Pinned` holds one, and a chrome with no reason
/// to be held is `Shown`, so the empty set has no value to be constructed as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PinReasons(u8);

impl PinReasons {
    /// The set holding only `reason`.
    pub const fn of(reason: PinReason) -> Self {
        PinReasons(reason.bit())
    }

    /// This set and `reason`.
    pub const fn with(self, reason: PinReason) -> Self {
        PinReasons(self.0 | reason.bit())
    }

    /// This set without `reason`, or `None` when nothing is left to hold the chrome.
    pub const fn without(self, reason: PinReason) -> Option<Self> {
        let bits = self.0 & !reason.bit();
        if bits == 0 {
            None
        } else {
            Some(PinReasons(bits))
        }
    }

    /// Whether `reason` is in the set.
    pub const fn contains(self, reason: PinReason) -> bool {
        self.0 & reason.bit() != 0
    }

    /// The reasons in the set, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = PinReason> {
        PinReason::ALL
            .iter()
            .copied()
            .filter(move |reason| self.contains(*reason))
    }
}
