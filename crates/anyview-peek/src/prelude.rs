//! The names a caller of the peek uses: the one call that looks at a file, what it is told and
//! what it gives back, and the worker that runs it off the UI thread. `use
//! anyview_peek::prelude::*;` brings them in.
//!
//! What belongs here is the front door ([`look`], [`Peeking`]), the answer ([`AnyPeeked`],
//! [`Body`], [`Unavailable`]), the worker, and the pane with the parts it is asked to draw. The
//! per-format peeks, the registry and the helpers stay at the crate root.

pub use crate::{
    AnyPeeked, Body, NoStills, PeekError, PeekWorker, Peeking, StillSource, Unavailable,
    WorkerConfig, look,
};

#[cfg(feature = "pane")]
pub use crate::{Pane, Part, Parts};
