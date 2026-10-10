//! Text for the viewer: everything a person reads as characters, as blocking, effect-free
//! back ends that run on the caller's worker.
//!
//! - Plain text: `detect` finds the encoding, `TextCodec` decodes it and `TextLines` serves a
//!   window of lines of a file too large to hold, with `Needle` to find in it.
//! - Code: a `Highlighter` (built once by whoever owns it) turns lines into token classes
//!   (`CodeLines`, `TokenLine`), which the views map to colours.
//! - Markdown: `render` gives the HTML of a body and its outline; local images are read through
//!   a `LocalFiles` the caller injects (`DiskFiles`, or `NoFiles` for a peek).
//! - Tables and trees: `Table` for CSV and TSV, `Workbook` and `Sheet` for spreadsheets, `Tree`
//!   for JSON.
//! - The light tier: the `*Peek` types describe a file in a few lines without opening it whole.
//! - Exports: `plan_export` and `plan_print` plan what a text document is written or printed
//!   as, and `printable_html` is the page a renderer lays out.
//!
//! Nothing here spawns, reads the environment or the clock, or draws.
//!
//! ```
//! use anyview_text::{NoFiles, RenderEnv, render};
//!
//! let env = RenderEnv {
//!     base: None,
//!     files: &NoFiles,
//!     highlighter: None,
//! };
//! let page = render("# Title\n\nSome *text*.", &env);
//! assert!(page.html.contains("<em>text</em>"));
//! assert_eq!(page.outline.len(), 1);
//! ```

#![warn(missing_docs)]

mod bytes;
mod code;
mod encoding;
mod error;
mod escape;
mod export;
mod find;
mod lines;
mod markdown;
mod peek;
mod table;
mod tree;

pub use bytes::FileBytes;
pub use code::{
    CodeLines, Highlighter, SyntaxId, TOKEN_CLASS_PREFIX, TokenClass, TokenLine, TokenSpan,
    tokens_html,
};
pub use encoding::{Coverage, DETECT_BYTES, Detected, TextCodec, detect};
pub use error::TextError;
pub use export::{plan_export, plan_print, printable_html};
pub use find::{ByteOffset, FindHit, MAX_HITS, Needle};
pub use lines::{LineCount, TextLines};
pub use markdown::{
    Anchor, DiskFiles, Heading, HeadingLevel, LocalFiles, NoFiles, RenderEnv, Rendered, render,
};
pub use peek::{
    CodePeek, CodePeeked, MarkdownPeek, MarkdownPeeked, PEEK_LINES, PlainPeek, PlainPeeked,
    TablePeek, TablePeeked, TableSource, Tally, TreePeek, TreePeeked,
};
pub use table::{
    CharWidth, ColumnCount, HeaderMode, RowCount, RowIndex, SHEET_ROWS, Separator, Sheet,
    TABLE_BYTES, TABLE_ROWS, Table, WORKBOOK_BYTES, WORKBOOK_CELLS, Workbook,
};
pub use tree::{ChildCount, NodeKind, Openness, RowLabel, Tree, TreeRow, VisibleRow};
