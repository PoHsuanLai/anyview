//! The light tier for text: `Peek` implementations for plain text, code, Markdown, tables and
//! trees. Each reads at most the byte budget from the start of the file and shows at most
//! [`PEEK_LINES`] lines or rows.

mod code;
pub(crate) mod head;
mod markdown;
mod plain;
mod table;
mod tally;
mod tree;

pub use code::{CodePeek, CodePeeked};
pub use head::PEEK_LINES;
pub use markdown::{MarkdownPeek, MarkdownPeeked};
pub use plain::{PlainPeek, PlainPeeked};
pub use table::{TablePeek, TablePeeked, TableSource};
pub use tally::Tally;
pub use tree::{TreePeek, TreePeeked};
