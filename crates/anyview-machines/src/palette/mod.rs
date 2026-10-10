//! The ⌘K palette: quire's `PaletteState<Command>` (the query, the selection, what Enter runs) with
//! what anyview adds to it, a scope, kept beside it in the viewer and stepped by [`step_with_scope`].

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{
    HitList, Palette, PaletteIn, PaletteIndex, PaletteMove, PaletteOut, PaletteParams, PaletteScope,
};
pub(crate) use step::step_with_scope;
