//! Where the sound goes: the trait the player plays through, the sound card behind it, and the
//! output that plays nothing.

mod card;
mod silent;

pub use card::{CardOutput, card_present};
pub use silent::SilentOutput;

use super::format::StreamFormat;
use super::pipe::Pipe;
use crate::error::MediaError;
use std::sync::Arc;

/// A sound output the player drains its queue into. The real one is the sound card; tests give
/// one that the test itself drains, so no device is opened.
pub trait SoundOutput {
    /// The format the output will take, as near `wanted` as it can: the sound is converted to it.
    fn negotiate(&mut self, wanted: StreamFormat) -> Result<StreamFormat, MediaError>;

    /// Start calling `pipe.fill` for sound, in the format `negotiate` answered. The output is
    /// running; the pipe's gate decides whether it is given sound or silence.
    fn start(&mut self, pipe: Arc<Pipe>) -> Result<(), MediaError>;

    /// Stop calling `fill` until `resume`: a pause that lets the device go idle.
    fn pause(&mut self);

    /// Call `fill` again.
    fn resume(&mut self);
}
