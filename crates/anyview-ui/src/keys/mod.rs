//! Key routing: which region of the viewer gets a key. One pure function over the states of the
//! regions that can claim keys.

mod model;
mod route;
#[cfg(test)]
mod tests;

pub use model::{Regions, Route};
pub use route::route;
