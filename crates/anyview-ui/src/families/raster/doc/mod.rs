//! A decoded picture on the GPU, and the back end that puts it there. Decoding and the upload
//! happen on a worker; the window holds the small document that names the texture.

mod build;
mod decode;
mod first;
mod types;

pub(super) use first::first_frame;
pub use types::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterOpen};
