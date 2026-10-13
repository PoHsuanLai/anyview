//! What a peek may spend.

use crate::source::ByteLen;
use crate::units::PixelArea;
use std::time::Duration;

/// The limits a peek stays inside, so the launcher pane stays light whatever file it is pointed
/// at. The values come from settings; this type only carries them. It is `#[non_exhaustive]`, so a
/// limit added later breaks no caller: start from [`PeekBudget::default`] and change a limit with
/// the `with_*` methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct PeekBudget {
    /// The most bytes it may read from the file.
    pub bytes: ByteLen,
    /// The most pixels it may decode.
    pub pixels: PixelArea,
    /// The longest it may take.
    pub time: Duration,
}

impl PeekBudget {
    /// What the launcher's pane allows: 64 MiB read, a million pixels decoded, half a second. It is
    /// what [`Default`] answers, as a constant for a `const` that changes only some limits.
    pub const DEFAULT: PeekBudget = PeekBudget {
        bytes: ByteLen(64 * 1024 * 1024),
        pixels: PixelArea(1_000_000),
        time: Duration::from_millis(500),
    };

    /// This budget reading no more than `bytes`.
    #[must_use]
    pub const fn with_bytes(self, bytes: ByteLen) -> Self {
        PeekBudget { bytes, ..self }
    }

    /// This budget decoding no more than `pixels`.
    #[must_use]
    pub const fn with_pixels(self, pixels: PixelArea) -> Self {
        PeekBudget { pixels, ..self }
    }

    /// This budget taking no longer than `time`.
    #[must_use]
    pub const fn with_time(self, time: Duration) -> Self {
        PeekBudget { time, ..self }
    }
}

impl Default for PeekBudget {
    fn default() -> Self {
        PeekBudget::DEFAULT
    }
}
