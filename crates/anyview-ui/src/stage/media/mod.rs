//! The media stage: opening, playing, paused, scrubbing, ended, failed, over what the player
//! reports.

mod event;
mod model;
mod playing;
mod scrubbing;
mod step;
#[cfg(test)]
mod tests;

pub use event::{EndReason, FrameDirection, Pace, PlayerCommand, PlayerEvent, TrackKind};
pub use model::{AfterScrub, MediaError, MediaIn, MediaOut, MediaParams, MediaStage};
