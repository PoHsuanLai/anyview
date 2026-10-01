//! The text stage: reading text, code and Markdown, with find, wrapping and a rendered or
//! source view.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{TextIn, TextOut, TextParams, TextPlace, TextStage, TextView, TextViews, Wrap};
