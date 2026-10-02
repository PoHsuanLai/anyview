//! The text stage: reading text, code and Markdown, with find, wrapping and a rendered or
//! source view.

mod model;
mod step;
mod steps;
#[cfg(test)]
mod tests;

pub use model::{
    LineTotal, PageLines, TextExtent, TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep,
    TextView, TextViews, Wrap,
};
