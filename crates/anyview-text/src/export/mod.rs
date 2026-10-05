//! Text exports: the jobs a choice becomes, and the HTML a printed document is laid out from.
//!
//! A Markdown, code or plain-text file becomes a PDF through an HTML page that quire prints: the
//! page is built here, the printing is the caller's (`ds_blitz::pdf`).

mod html;
mod plan;

pub use html::printable_html;
pub use plan::{plan_export, plan_print};
