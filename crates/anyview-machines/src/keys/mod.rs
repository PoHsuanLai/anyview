//! Key routing: which region of the viewer gets a key. One pure function over the states of the
//! regions that can claim keys.

mod act;
mod model;
mod route;
#[cfg(test)]
mod tests;

pub use act::Act;
pub use act::{app, rows, standing};
pub use model::{Chords, Press, Regions, Route};
pub use route::route;
