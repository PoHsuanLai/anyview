//! The contract between the viewer's machines and the threads that serve them: what a back end
//! offers a worker (`Backend`), how a worker is told to give up (`Stop`), and how a result is
//! matched to the load that asked for it (`Ticket`, `Ticketed`). Pure: the binary owns every
//! thread and every clock, and hands the instant in; the one exception is `Interrupt`, the question
//! a long loop asks to end early, which reads the clock when the thing it asks about has a limit.

mod backend;
mod interrupt;
mod stop;
mod ticket;

#[cfg(test)]
mod tests;

pub use backend::Backend;
pub use interrupt::Interrupt;
pub use stop::{Stop, StopState};
pub use ticket::{Ticket, Ticketed};
