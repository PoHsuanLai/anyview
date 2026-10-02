use super::locked;
use crate::stacking::{Stacking, StackingOutcome, WindowStacking};
use ds_core::word::Word;
use std::sync::{Arc, Mutex};

/// Whether the fake's desktop can keep a window above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StackingSupport {
    /// It can.
    Supported,
    /// It cannot.
    Unsupported,
}

/// A [`WindowStacking`] that records the requests and answers by its support.
#[derive(Debug, Clone)]
pub struct FakeStacking {
    support: StackingSupport,
    requested: Arc<Mutex<Vec<Stacking>>>,
}

impl FakeStacking {
    /// A desktop with `support`.
    pub fn with(support: StackingSupport) -> Self {
        FakeStacking {
            support,
            requested: Arc::default(),
        }
    }

    /// Every request, oldest first.
    pub fn requested(&self) -> Vec<Stacking> {
        locked(&self.requested).clone()
    }
}

impl WindowStacking for FakeStacking {
    fn request(&self, stacking: Stacking) -> StackingOutcome {
        locked(&self.requested).push(stacking);
        match (stacking, self.support) {
            (Stacking::Normal, _) | (Stacking::KeepAbove, StackingSupport::Supported) => {
                StackingOutcome::Applied
            }
            (Stacking::KeepAbove, StackingSupport::Unsupported) => StackingOutcome::Unsupported,
        }
    }
}
