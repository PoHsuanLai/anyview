//! What one poll of the player found.

use crate::event::MediaEvent;

/// Whether the player rewrote its texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// The same texels as before.
    Unchanged,
    /// A new frame is in the texture.
    Rewritten,
}

/// What a poll found: the events, in order, and whether a new frame was drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The player's notifications, translated.
    pub events: Vec<MediaEvent>,
    /// Whether the picture changed.
    pub frame: Frame,
}

impl Report {
    /// A poll that found nothing.
    pub fn quiet() -> Report {
        Report {
            events: Vec::new(),
            frame: Frame::Unchanged,
        }
    }
}
