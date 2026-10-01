//! Text for the viewer.

mod bytes;
mod code;
mod encoding;
mod error;
mod escape;
mod lines;
mod markdown;

pub use bytes::{ByteSource, FileBytes, HeldBytes};
pub use code::{
    CodeLines, Highlighter, SyntaxId, TOKEN_CLASS_PREFIX, TokenClass, TokenLine, TokenSpan,
    tokens_html,
};
pub use encoding::{Coverage, DETECT_BYTES, Detected, TextCodec, detect};
pub use error::TextError;
pub use lines::{LineCount, TextLines};
pub use markdown::{
    Anchor, Heading, HeadingLevel, LocalFiles, NoFiles, RenderEnv, Rendered, render,
};
