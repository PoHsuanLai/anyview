//! Delivery to the UI thread: any thread posts, the UI thread drains, and the UI thread is woken
//! at most once until it has drained.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Wakes the UI thread so it drains its mailbox. The views supply one built on the window's
/// event-loop proxy or `TextureHandle::redraw`; it may be called from any thread and must only
/// wake.
pub trait UiWaker: Send + Sync + 'static {
    /// Ask the UI thread to drain its mailbox soon.
    fn wake(&self);
}

struct Inner<T> {
    queue: Mutex<VecDeque<T>>,
    /// Set from the first post after a drain until the next drain begins; the waker runs only on
    /// that first post, so a burst of results costs one wake.
    wake_sent: AtomicBool,
    waker: Box<dyn UiWaker>,
}

/// The UI thread's end: what other threads posted, in the order it arrived.
pub struct Mailbox<T> {
    inner: Arc<Inner<T>>,
}

/// The posting end, cloned to every thread that delivers.
pub struct Outbox<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Clone for Outbox<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> fmt::Debug for Mailbox<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mailbox").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for Outbox<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Outbox").finish_non_exhaustive()
    }
}

impl<T: Send> Mailbox<T> {
    /// A mailbox and its outbox; `waker` is called when the first message arrives after a drain.
    pub fn new(waker: impl UiWaker) -> (Mailbox<T>, Outbox<T>) {
        let inner = Arc::new(Inner {
            queue: Mutex::new(VecDeque::new()),
            wake_sent: AtomicBool::new(false),
            waker: Box::new(waker),
        });
        (
            Mailbox {
                inner: Arc::clone(&inner),
            },
            Outbox { inner },
        )
    }

    /// Everything posted so far, oldest first. The UI thread calls it whenever it is woken and
    /// applies every message: a wake is not sent again until this runs.
    pub fn drain(&self) -> Vec<T> {
        self.inner.wake_sent.store(false, Ordering::SeqCst);
        let mut queue = self
            .inner
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        queue.drain(..).collect()
    }
}

impl<T: Send> Outbox<T> {
    /// Posts `message` and wakes the UI thread unless a wake is already waiting for it.
    pub fn send(&self, message: T) {
        self.inner
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(message);
        if !self.inner.wake_sent.swap(true, Ordering::SeqCst) {
            self.inner.waker.wake();
        }
    }
}
