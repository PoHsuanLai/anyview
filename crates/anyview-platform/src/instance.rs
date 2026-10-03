//! One viewer process: the second launch hands what it was asked to the first and leaves.

use crate::error::PlatformError;
use anyview_core::{FilePath, Resume, Sequence};
use std::any::Any;
use std::future::Future;
use tokio::sync::mpsc::UnboundedReceiver;

/// What a launch asks of the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Open these files in the window; none means "show the window".
    Open(Vec<FilePath>),
    /// Show this file in the quick-look window.
    Peek(FilePath),
    /// Open this file and start it playing.
    Play(FilePath),
    /// Open one file the way the launcher's pane left it.
    Handoff(Handoff),
}

/// A file the launcher's pane was showing when the person asked to open it: where the pane had
/// it, and the search results it was chosen from, so the viewer carries on from there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handoff {
    /// The file.
    pub file: FilePath,
    /// Where the pane had it. `Resume::Nothing` says the pane held no place of its own, and the
    /// viewer then continues where the person last left the file.
    pub resume: Resume,
    /// The results ← and → walk, with `file` among them; `None` when the file came from no list.
    pub sequence: Option<Sequence>,
}

/// The right to be the viewer: the requests other launches forward arrive here, and dropping it
/// gives the name up.
pub struct Primary {
    requests: UnboundedReceiver<Request>,
    /// The registration that keeps the name; opaque so a fake holds nothing.
    _held: Box<dyn Any + Send>,
}

impl Primary {
    pub(crate) fn new(requests: UnboundedReceiver<Request>, held: Box<dyn Any + Send>) -> Self {
        Primary {
            requests,
            _held: held,
        }
    }

    /// The next request another launch forwarded, or `None` when nothing can arrive any more.
    pub async fn next(&mut self) -> Option<Request> {
        self.requests.recv().await
    }
}

impl std::fmt::Debug for Primary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Primary").finish_non_exhaustive()
    }
}

/// Who handles a launch.
#[derive(Debug)]
pub enum Claim {
    /// No viewer was running: this process is it, and acts on its own request itself.
    Primary(Primary),
    /// A viewer was running and has the request; this process has nothing left to do.
    Forwarded,
}

/// Single instance: own the viewer's name, or forward to whoever does.
pub trait Instance {
    /// Become the viewer, or forward `request` to the one that is. A refusal to forward is an
    /// error: the caller then runs on as a second viewer rather than dropping the request.
    fn claim(&self, request: &Request)
    -> impl Future<Output = Result<Claim, PlatformError>> + Send;
}
