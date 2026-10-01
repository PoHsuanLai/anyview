//! Structured text shown as a tree.

use super::family::Family;
use ds_core::word::Word;

/// A format shown as a tree of values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TreeFormat {
    /// One JSON document.
    Json,
    /// One JSON value per line.
    JsonLines,
}

impl Family for TreeFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            TreeFormat::Json => &["json"],
            TreeFormat::JsonLines => &["jsonl", "ndjson"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            TreeFormat::Json => "application/json",
            TreeFormat::JsonLines => "application/x-ndjson",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_well_formed() {
        crate::kind::family::tests::assert_well_formed::<TreeFormat>();
    }
}
