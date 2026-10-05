//! Text for the viewer.

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

pub use bytes::{ByteSource, FileBytes, HeldBytes};
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
    TablePeek, TablePeeked, Tally, TreePeek, TreePeeked,
};
pub use table::{ColumnCount, HeaderMode, RowCount, RowIndex, Table};
pub use tree::{ChildCount, NodeKind, RowLabel, Tree, TreePath, TreeRow};
