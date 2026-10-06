//! The context menu: a secondary click, or the Menu key, over the content opens the rows a Mac's
//! context menu shows for the open file. The menu owns where it is open; the rows come from the
//! palette's commands (`entries`), and the keys inside it are the menu component's.

mod entries;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub(crate) use entries::entries;
pub use model::{
    ContextEntry, ContextIn, ContextMenu, ContextOut, ContextParams, ContextPick, Spot,
};
