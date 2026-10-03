//! Delimited tables.

use super::family::Family;
use ds_core::word::Word;

/// The character that separates a table's cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Delimiter {
    /// Comma separated values.
    Comma,
    /// Tab separated values.
    Tab,
}

impl Family for Delimiter {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Delimiter::Comma => &["csv"],
            Delimiter::Tab => &["tsv", "tab"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            Delimiter::Comma => "text/csv",
            Delimiter::Tab => "text/tab-separated-values",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_well_formed() {
        crate::kind::family::tests::assert_well_formed::<Delimiter>();
    }
}
