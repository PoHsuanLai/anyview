use super::locked;
use crate::error::PlatformError;
use crate::media::{MediaControl, MediaSession, MediaState};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// A [`MediaSession`] that records what is published and delivers the controls a test presses.
#[derive(Debug)]
pub struct FakeMediaSession {
    published: Arc<Mutex<Vec<MediaState>>>,
    sender: UnboundedSender<MediaControl>,
    controls: UnboundedReceiver<MediaControl>,
}

impl FakeMediaSession {
    /// A session with nothing published.
    pub fn new() -> Self {
        let (sender, controls) = unbounded_channel();
        FakeMediaSession {
            published: Arc::default(),
            sender,
            controls,
        }
    }

    /// Every state published, oldest first.
    pub fn published(&self) -> Vec<MediaState> {
        locked(&self.published).clone()
    }

    /// Press `control` as the desktop would; `next_control` yields it.
    pub fn press(&self, control: MediaControl) {
        // The receiver lives in this value, so the send cannot find it closed.
        let _ = self.sender.send(control);
    }
}

impl Default for FakeMediaSession {
    fn default() -> Self {
        FakeMediaSession::new()
    }
}

impl MediaSession for FakeMediaSession {
    async fn publish(&self, state: &MediaState) -> Result<(), PlatformError> {
        locked(&self.published).push(state.clone());
        Ok(())
    }

    async fn next_control(&mut self) -> Option<MediaControl> {
        self.controls.recv().await
    }
}
