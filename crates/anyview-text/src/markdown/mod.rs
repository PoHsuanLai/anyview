//! Markdown to HTML for Blitz's sealed `iframe srcdoc`, with an outline for the side panel.
//!
//! CommonMark with GFM tables, task lists and strikethrough. Raw HTML in the document is shown as
//! text, never passed through. Links keep only web, mail and relative targets. Local images are
//! read through an injected [`LocalFiles`] and written as `data:` URLs, the only kind a sealed
//! frame loads; an image that cannot be inlined is shown as its alt text. Fenced code is
//! highlighted into `tok-…` classes when a [`Highlighter`](crate::Highlighter) is supplied.

mod events;
mod images;
mod links;
mod outline;

#[cfg(test)]
mod tests;

pub use events::RenderEnv;
pub use images::{DiskFiles, LocalFiles, NoFiles};
pub use outline::{Anchor, Heading, HeadingLevel};

use pulldown_cmark::{Options, Parser, html};

/// A rendered document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The HTML of the body: no `<html>` or `<head>`, no script, no raw HTML of the source.
    pub html: String,
    /// The headings in document order; each anchor is the `id` of its element in `html`.
    pub outline: Vec<Heading>,
}

/// The Markdown extensions the viewer reads: GFM tables, task lists and strikethrough.
fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
}

/// `source` as HTML and an outline.
#[must_use]
pub fn render(source: &str, env: &RenderEnv<'_>) -> Rendered {
    let parsed: Vec<_> = Parser::new_ext(source, options()).collect();
    let (events, outline) = events::prepare(parsed, env);
    let mut out = String::with_capacity(source.len() * 2);
    html::push_html(&mut out, events.into_iter());
    Rendered { html: out, outline }
}
