//! The chrome's states, inputs and outputs, and its timing.

use super::pins::{PinReason, PinReasons};
use ds_core::time::stamp::Stamp;
use ds_core::vocab::Shown;
use std::time::Duration;

/// Whether the chrome is on screen, and what will change that.
///
/// A state that waits on the clock holds the instant it waits for, so `wake()` needs no
/// settings: the setting that applied when the state was entered decides how long it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Chrome {
    /// Off screen.
    #[default]
    Hidden,
    /// Fading in; the fade ends at `until`.
    Revealing { since: Stamp, until: Stamp },
    /// On screen and counting idle time: it hides at `hide_at`, which is `idle_from` plus the
    /// hide delay.
    Shown { idle_from: Stamp, hide_at: Stamp },
    /// Held on screen by at least one reason; no timer runs.
    Pinned { by: PinReasons },
    /// Fading out; the fade ends at `until`.
    Hiding { since: Stamp, until: Stamp },
}

/// Where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    /// Over the content: moving here shows the chrome and restarts the idle count.
    Content,
    /// Over the capsule or the titlebar: the chrome stays.
    Capsule,
}

/// What moves the chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeIn {
    /// The pointer moved in `Zone`.
    PointerMoved(Zone),
    /// The pointer left the window.
    PointerLeft,
    /// A reason to hold the chrome began.
    Pin(PinReason),
    /// A reason to hold the chrome ended.
    Unpin(PinReason),
    /// The time `wake()` named has come.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for ChromeIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        ChromeIn::Elapsed
    }
}

/// What the chrome wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeOut {
    /// Fade to `to` over `over`; carried out by the view with `ds-motion`.
    Fade { to: Shown, over: Duration },
}

/// The chrome's timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromeParams {
    /// Idle time before the chrome hides (setting `viewer.chrome.hide_after`; quire
    /// design/20 section 2.6 proposes 2000 ms).
    pub hide_after: Duration,
    /// The fade in: quire's motion token `--t-quick`, 150 ms.
    pub reveal: Duration,
    /// The fade out: `--t-quick`, 150 ms.
    pub hide: Duration,
}

impl ChromeParams {
    /// The idle delay quire's design/20 section 2.6 proposes.
    pub const HIDE_AFTER: Duration = Duration::from_millis(2000);
    /// `--t-quick`.
    pub const QUICK: Duration = Duration::from_millis(150);
}

impl Default for ChromeParams {
    fn default() -> Self {
        ChromeParams {
            hide_after: Self::HIDE_AFTER,
            reveal: Self::QUICK,
            hide: Self::QUICK,
        }
    }
}
