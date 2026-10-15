//! A sheet on screen: quire's `VirtualTable`, a header over the rows it mounts. Only the rows
//! near the viewport exist as elements and each is asked of the sheet by `(row, column)`, which
//! parses the window of the file it is in, so a sheet of any size costs a screenful. A row-number gutter leads the columns;
//! numeric columns sit at the trailing edge; the header's presses sort the rows for the eye
//! only (the stage machine keeps the sort, the file is never reordered). The gutter and the
//! first column are held in place while the others scroll sideways under them, and rows are
//! picked as in Finder's list: a click, Command-click, Shift-click, Shift and the arrows, Select
//! All, with Copy putting them on the clipboard as tab-separated lines (`pick`).

mod content;
mod look;
mod scroll;

pub(super) use content::TableContent;
pub(super) use look::{page_of, title};
