//! The picture on screen: a clipped room holding one `TextureLayer` at the box geometry placed,
//! and the pointer and gestures that move it. The view reads the machine's state and sends it
//! inputs; every decision (what a pinch means at this zoom, where a double-click goes) is the
//! machine's.

mod content;
mod layout;
mod modes;

pub(super) use content::RasterContent;
pub(crate) use modes::PointerModes;
