//! The pages on screen: a clipped room holding the pages in view, each a box of tiles, and the
//! pointer and gestures that move it. The view reads the machine's state and sends it inputs; the
//! decisions (which page is at the top, what a zoom step means) are the machine's, and the tiles
//! are asked of the workers here because only the view knows what the room shows.

mod content;
mod hits;
mod pointer;

pub(super) use content::PdfContent;
