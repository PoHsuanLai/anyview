//! What the media thread needs of a player, whichever plays: mpv through [`crate::Driver`], or the
//! built-in audio player through `BuiltinDriver`. The actor holds one behind this trait and nothing
//! it does depends on which.

use crate::command::MediaCommand;
use crate::event::MediaEvent;

/// Whether the driver goes on after an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuation {
    /// Wait for the next instruction or wake.
    Keep,
    /// End the session: the actor's thread quits.
    Close,
}

/// What an instruction caused.
#[derive(Debug, Clone, PartialEq)]
pub struct Handled {
    /// Events, in order.
    pub events: Vec<MediaEvent>,
    /// Whether the session goes on.
    pub then: Continuation,
}

/// One player on the media thread: it is sent instructions and asked to look to itself, and says
/// what happened as events. Nothing here blocks the thread for long, and nothing spawns.
pub trait MediaDriver {
    /// Carry out one instruction.
    fn command(&mut self, command: MediaCommand) -> Handled;

    /// The player asked for attention (its wake ran): drain it and say what changed.
    fn woken(&mut self) -> Vec<MediaEvent>;
}
