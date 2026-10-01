//! Text a person typed: the palette's query, a find query, a new file name.

use std::borrow::Cow;

/// Text typed into a field. A static string is held without a copy, which lets a state table
/// spell a query as a constant; typed text is owned.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TypedText(Cow<'static, str>);

impl TypedText {
    /// The empty text.
    pub const EMPTY: TypedText = TypedText(Cow::Borrowed(""));

    /// Typed text, owned.
    pub fn new(text: impl Into<String>) -> Self {
        TypedText(Cow::Owned(text.into()))
    }

    /// A literal, held without a copy.
    pub const fn from_static(text: &'static str) -> Self {
        TypedText(Cow::Borrowed(text))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether nothing was typed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
