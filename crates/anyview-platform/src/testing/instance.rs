use super::locked;
use crate::error::PlatformError;
use crate::instance::{Claim, Instance, Primary, Request};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

/// What the fake plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeRole {
    /// No viewer runs: the claim succeeds.
    FirstLaunch,
    /// A viewer runs: the claim forwards.
    SecondLaunch,
}

/// An [`Instance`] that decides by its role, records every request it is asked about, and feeds
/// the requests a test sends to the [`Primary`] it hands out.
#[derive(Debug, Clone)]
pub struct FakeInstance {
    role: FakeRole,
    claimed: Arc<Mutex<Vec<Request>>>,
    sender: Sender<Request>,
    receiver: Arc<Mutex<Option<Receiver<Request>>>>,
}

impl FakeInstance {
    /// A fake in `role`.
    pub fn new(role: FakeRole) -> Self {
        let (sender, receiver) = channel();
        FakeInstance {
            role,
            claimed: Arc::default(),
            sender,
            receiver: Arc::new(Mutex::new(Some(receiver))),
        }
    }

    /// Every request given to `claim`, oldest first: what a second launch forwarded, or what the
    /// first launch asked for itself.
    pub fn claimed(&self) -> Vec<Request> {
        locked(&self.claimed).clone()
    }

    /// Deliver `request` to the [`Primary`], as another launch would.
    pub fn forward(&self, request: Request) {
        // A closed receiver means the primary was dropped; nobody is left to deliver to.
        let _ = self.sender.send(request);
    }
}

impl Instance for FakeInstance {
    async fn claim(&self, request: &Request) -> Result<Claim, PlatformError> {
        locked(&self.claimed).push(request.clone());
        match self.role {
            FakeRole::SecondLaunch => Ok(Claim::Forwarded),
            FakeRole::FirstLaunch => {
                let receiver = locked(&self.receiver).take();
                Ok(match receiver {
                    Some(receiver) => Claim::Primary(Primary::new(receiver, Box::new(()))),
                    // Claimed twice: the name is taken by the first claim.
                    None => Claim::Forwarded,
                })
            }
        }
    }
}
