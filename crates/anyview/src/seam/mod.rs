//! The one seam between the window's views and the program's threads: the views hand a [`Work`]
//! to [`Workers`], and this carries it out on the runtime's pool.
//!
//! A worker runs the work, which posts its `Done` to the window's own reply channel (a
//! `futures` channel whose receiver the window's UI task awaits, so the post wakes the window's
//! event loop through the task's waker: ds-blitz keeps its event-loop proxy to itself). What the
//! pool adds is the delivery of how each job ended, ticketed, through a [`Mailbox`]: a job that
//! was skipped or panicked posts no `Done`, and the program's report task learns of it there.

mod notice;
mod workforce;

#[cfg(test)]
mod tests;

pub use notice::{Notice, NoticeWaker, notices};
pub use workforce::{Settled, Workforce};
