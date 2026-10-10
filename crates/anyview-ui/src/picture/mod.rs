//! The edits made to the open picture and not yet saved: a crop, a resize, turns and mirrors, the
//! rectangle of a crop while it is drawn, and the steps that can be taken back. A pure state like
//! the pointer tool's; the file is written only when the host is asked to (⌘S).

mod crop;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use crop::{CropAspect, CropBox, CropGrip, CropLean, CropShape};
pub use model::{PictureEditIn, PictureEditing, PictureEdits};
