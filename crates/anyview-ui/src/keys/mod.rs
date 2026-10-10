//! Key routing: which region of the viewer gets a key. One pure function over the states of the
//! regions that can claim keys.

mod act;
mod model;
mod route;
#[cfg(test)]
mod tests;

pub use act::Act;
pub(crate) use act::{app, rows};
pub use model::{Press, Regions, Route};
pub use route::route;
