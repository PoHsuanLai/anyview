//! What a peek may spend.

use crate::source::ByteLen;
use crate::units::PixelArea;
use std::time::Duration;

/// The limits a peek stays inside, so the launcher pane stays light whatever file it is pointed
/// at. The values come from settings; this type only carries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeekBudget {
    /// The most bytes it may read from the file.
    pub bytes: ByteLen,
    /// The most pixels it may decode.
    pub pixels: PixelArea,
    /// The longest it may take.
    pub time: Duration,
}
