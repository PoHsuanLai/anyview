//! The session before a file is given.

use super::{Idle, Opening, Session, fault, options::AudioDriver};
use crate::error::MediaError;
use anyview_core::FilePath;

/// A session that would not take the file, and why. The session is returned so the caller can
/// try another.
#[derive(Debug)]
pub struct Refused {
    /// The session, still idle.
    pub session: Session<Idle>,
    /// What went wrong.
    pub error: MediaError,
}

impl Session<Idle> {
    /// A player on the window's `device` and `queue` that plays its sound on `audio`.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        audio: AudioDriver,
    ) -> Result<Session<Idle>, MediaError> {
        let options = mpv_wgpu_player::PlayerOptions {
            audio_output: audio.output(),
        };
        let player = mpv_wgpu_player::Player::new(device, queue, options).map_err(fault)?;
        Ok(Session {
            player,
            state: Idle,
        })
    }

    /// Hand `file` to mpv. It opens in the background: `Session<Opening>::poll` says when.
    pub fn open(self, file: &FilePath) -> Result<Session<Opening>, Refused> {
        let Some(path) = file.as_path().to_str() else {
            return Err(Refused {
                session: self,
                error: MediaError::Player("the path is not valid UTF-8".to_owned()),
            });
        };
        match self.player.load(path) {
            Ok(()) => Ok(self.with_state(Opening)),
            Err(error) => Err(Refused {
                error: fault(error),
                session: self,
            }),
        }
    }
}
