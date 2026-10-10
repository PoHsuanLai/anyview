//! The text stage: reading text, code and Markdown, with find, wrapping and a rendered or
//! source view.

mod model;
mod step;
mod steps;
#[cfg(test)]
mod tests;
mod wrap;

pub use model::{
    Changes, EditFind, Editable, Edited, LineTotal, Outside, PageLines, TextExtent, TextIn,
    TextOut, TextParams, TextPlace, TextStage, TextStep, TextView, TextViews, Wrap,
};

pub(crate) use wrap::WrapChoices;
