//! The text stage: reading text, code and Markdown, with find, wrapping and a rendered or
//! source view.

mod command;
mod model;
mod step;
mod steps;
#[cfg(test)]
mod tests;
mod wrap;

pub(super) use command::input_for;
pub use model::{
    Changes, Editable, Edited, LineTotal, Outside, PageLines, TextExtent, TextIn, TextOut,
    TextParams, TextPlace, TextStage, TextStep, TextView, TextViews, Wrap,
};

pub use wrap::WrapChoices;
