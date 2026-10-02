//! Units: every quantity the viewer passes around has its own type, all integer so equality holds.

mod content_space;
mod dpi;
mod media;
mod orientation;
mod page;
mod pixels;
mod ratio;
mod zoom;

pub use content_space::{DocPoint, DocUnit, LineIndex};
pub use dpi::Dpi;
pub use media::{Bitrate, ChapterIndex, MediaLength, MediaTime, Speed, TimeRange, Volume};
pub use orientation::{Axis, QuarterTurn};
pub use page::{PageCount, PageIndex, PageRange, PageSelection};
pub use pixels::{PixelArea, PixelLen, PixelSize};
pub use ratio::{Percent, Permille, Quality};
pub use zoom::Zoom;
