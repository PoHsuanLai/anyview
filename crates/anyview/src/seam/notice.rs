//! Telling the program about the jobs that did not finish: the wake that says the mailbox has
//! something, and the report of what it held.

use super::workforce::{Settled, Workforce};
use crate::runtime::{JobOutcome, UiWaker};
use anyview_core::work::Ticket;
use std::sync::Arc;
use tokio::sync::Notify;

/// A job that never posted a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// It was stopped before a worker took it up.
    Skipped(Ticket),
    /// It panicked, with the panic's message.
    Panicked(Ticket, String),
}

/// Wakes the task that reads the mailbox. Cloning shares the wake.
#[derive(Debug, Clone, Default)]
pub struct NoticeWaker {
    wake: Arc<Notify>,
}

impl NoticeWaker {
    /// Resolves once a job has ended since the last time it did.
    pub async fn woken(&self) {
        self.wake.notified().await;
    }
}

impl UiWaker for NoticeWaker {
    fn wake(&self) {
        self.wake.notify_one();
    }
}

/// What the ended jobs say went wrong; a job that ran to its end says nothing.
pub fn notices(ended: Vec<Settled>) -> Vec<Notice> {
    ended
        .into_iter()
        .filter_map(|ended| match ended.value {
            JobOutcome::Done(()) => None,
            JobOutcome::Skipped => Some(Notice::Skipped(ended.ticket)),
            JobOutcome::Panicked(panic) => Some(Notice::Panicked(ended.ticket, panic.message)),
        })
        .collect()
}

impl Workforce {
    /// The notices of the jobs that ended since the last call.
    pub fn notices(&self) -> Vec<Notice> {
        notices(self.settled())
    }
}
